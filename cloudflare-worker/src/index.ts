export interface Env {
  ENVIRONMENT: string;
  DB?: D1Database;
}

interface UserRecord {
  id: string;
  username: string;
  email: string;
  password_hash: string;
  salt: string;
  role: string;
  badge: string;
  skin_data?: string | null;
  skin_model: string;
  cape_data?: string | null;
  created_at: number;
  updated_at: number;
}

interface SessionRecord {
  token: string;
  user_id: string;
  created_at: number;
  expires_at: number;
}

// In-memory fallback stores when D1 is not attached
const memUsers = new Map<string, UserRecord>();
const memSessions = new Map<string, SessionRecord>();
const memSync = new Map<string, { settings?: string; instances?: string; updatedAt: number }>();

// Chat & Presence in-memory stores
const messages: Array<{
  id: string;
  channelId: string;
  sender: { id: string; username: string; status: string; avatarUrl?: string };
  text: string;
  timestamp: number;
}> = [];
const userPresence = new Map<string, { status: string; serverIp?: string; port?: number; updatedAt: number }>();

const CORS_HEADERS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type, Authorization',
};

// Anti-Disposable / Anti-Temp Email Domain Blacklist
const DISPOSABLE_EMAIL_DOMAINS = new Set([
  '10minutemail.com',
  '10minutemail.net',
  'temp-mail.org',
  'tempmail.com',
  'tempmail.net',
  'guerrillamail.com',
  'guerrillamail.net',
  'guerrillamail.biz',
  'guerrillamailblock.com',
  'mailinator.com',
  'yopmail.com',
  'throwawaymail.com',
  'fakemail.net',
  'trashmail.com',
  'trashmail.net',
  'trashmail.me',
  'getairmail.com',
  'dispostable.com',
  'crazymailing.com',
  'nada.ltd',
  'mohmal.com',
  'emailondeck.com',
  'sharklasers.com',
  'grr.la',
  'burnermail.io',
  'dropmail.me',
  'tempail.com',
  'mytemp.email',
  'fakemailgenerator.com',
  'zillamail.com',
  'mailsac.com',
  'spambox.us',
  'inboxbear.com',
  'generator.email',
  'disposablemail.com',
  'generator.email',
  'crazymailing.com',
  'armyspy.com',
  'cuvox.de',
  'dayrep.com',
  'fleckens.hu',
  'gustr.com',
  'jourrapide.com',
  'rhyta.com',
  'superrito.com',
  'teleworm.us',
  'tinmail.net',
]);

function isDisposableEmail(email: string): boolean {
  const parts = email.toLowerCase().trim().split('@');
  if (parts.length !== 2) return true;
  const domain = parts[1];
  if (DISPOSABLE_EMAIL_DOMAINS.has(domain)) return true;

  // Check subdomains (e.g. *.mailinator.com)
  for (const blacklisted of DISPOSABLE_EMAIL_DOMAINS) {
    if (domain.endsWith('.' + blacklisted)) return true;
  }
  return false;
}

function isValidEmail(email: string): boolean {
  const emailRegex = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;
  return emailRegex.test(email);
}

// Anti-Profanity & Swear Word Filter
const PROFANITY_PATTERNS = [
  'کیر', 'کص', 'کسکش', 'کونکش', 'کونی', 'کون', 'جنده', 'مادرجنده', 'ننه جنده', 'دیوث', 'سکس', 'سکسی',
  'بیناموس', 'بی ناموس', 'حرومزاده', 'حرامزاده', 'لاشی', 'پدرسگ', 'پدر سگ', 'خواهرکسه', 'خارکسه', 'خارکسته',
  'خایه', 'خایه مال', 'ساک زدن', 'کسخول', 'کسخل', 'کوس', 'کوست', 'چوچول', 'شاش', 'عن', 'گوه',
  'مادرقحبه', 'قحبه', 'سیکتیر', 'سیک تیر', 'بکیرم', 'بکیر', 'بکصم', 'کسشر', 'کسشعر', 'کصشعر', 'کصشر',
  'kir', 'kos', 'koss', 'koon', 'jende', 'jendeh', 'dayoos', 'dayus', 'binamoos', 'haroomzade', 'lashi',
  'pedarsag', 'kharkose', 'khaye', 'shash', 'gooh', 'sik', 'siktir', 'koonkesh', 'koskesh',
  'fuck', 'fucking', 'bitch', 'asshole', 'dick', 'pussy', 'whore', 'slut', 'cunt', 'nigger', 'nigga'
];

function containsProfanity(text: string): boolean {
  if (!text) return false;
  const raw = text.toLowerCase();
  let normalized = raw
    .replace(/[يك]/g, (c) => (c === 'ي' ? 'ی' : 'ک'))
    .replace(/[\u200B-\u200D\uFEFF]/g, '')
    .replace(/[._\-*#@!+=~`|\\/:;,?^%$()[\]{}<>"]/g, '');

  const collapsed = normalized.replace(/(.)\1{2,}/g, '$1$1');

  for (const pattern of PROFANITY_PATTERNS) {
    if (normalized.includes(pattern) || collapsed.includes(pattern)) {
      return true;
    }
  }

  const words = raw.split(/\s+/);
  for (const word of words) {
    const cleanWord = word.replace(/[^\p{L}\p{N}]/gu, '');
    for (const pattern of PROFANITY_PATTERNS) {
      if (cleanWord === pattern) {
        return true;
      }
    }
  }

  return false;
}

// PBKDF2 Password Hashing
async function hashPassword(password: string, saltHex: string): Promise<string> {
  const encoder = new TextEncoder();
  const passKey = await crypto.subtle.importKey(
    'raw',
    encoder.encode(password),
    { name: 'PBKDF2' },
    false,
    ['deriveBits', 'deriveKey']
  );

  const saltBytes = new Uint8Array(
    saltHex.match(/.{1,2}/g)!.map((byte) => parseInt(byte, 16))
  );

  const derivedBits = await crypto.subtle.deriveBits(
    {
      name: 'PBKDF2',
      salt: saltBytes,
      iterations: 100000,
      hash: 'SHA-256',
    },
    passKey,
    256
  );

  return Array.from(new Uint8Array(derivedBits))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

function generateRandomHex(byteCount: number): string {
  const arr = new Uint8Array(byteCount);
  crypto.getRandomValues(arr);
  return Array.from(arr)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

// Database helper functions
async function getUserByUsernameOrEmail(env: Env, identifier: string): Promise<UserRecord | null> {
  const clean = identifier.trim().toLowerCase();
  if (env.DB) {
    try {
      const res = await env.DB.prepare(
        'SELECT * FROM users WHERE LOWER(username) = ?1 OR LOWER(email) = ?1 LIMIT 1'
      )
        .bind(clean)
        .first<UserRecord>();
      if (res) return res;
    } catch (e) {
      console.error('D1 query error:', e);
    }
  }

  // Memory fallback
  for (const user of memUsers.values()) {
    if (user.username.toLowerCase() === clean || user.email.toLowerCase() === clean) {
      return user;
    }
  }
  return null;
}

async function getUserById(env: Env, id: string): Promise<UserRecord | null> {
  if (env.DB) {
    try {
      const res = await env.DB.prepare('SELECT * FROM users WHERE id = ?1 LIMIT 1')
        .bind(id)
        .first<UserRecord>();
      if (res) return res;
    } catch (e) {
      console.error('D1 query error:', e);
    }
  }
  return memUsers.get(id) || null;
}

async function insertUser(env: Env, user: UserRecord): Promise<void> {
  if (env.DB) {
    try {
      await env.DB.prepare(
        'INSERT INTO users (id, username, email, password_hash, salt, role, badge, skin_data, skin_model, cape_data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)'
      )
        .bind(
          user.id,
          user.username,
          user.email,
          user.password_hash,
          user.salt,
          user.role,
          user.badge,
          user.skin_data || null,
          user.skin_model,
          user.cape_data || null,
          user.created_at,
          user.updated_at
        )
        .run();
    } catch (e) {
      console.error('D1 insert error:', e);
    }
  }
  memUsers.set(user.id, user);
}

async function updateUserSkin(
  env: Env,
  userId: string,
  skinData: string,
  model: string,
  capeData?: string | null
): Promise<void> {
  const now = Date.now();
  if (env.DB) {
    try {
      await env.DB.prepare(
        'UPDATE users SET skin_data = ?1, skin_model = ?2, cape_data = ?3, updated_at = ?4 WHERE id = ?5'
      )
        .bind(skinData, model, capeData || null, now, userId)
        .run();
    } catch (e) {
      console.error('D1 skin update error:', e);
    }
  }
  const u = memUsers.get(userId);
  if (u) {
    u.skin_data = skinData;
    u.skin_model = model;
    u.cape_data = capeData || null;
    u.updated_at = now;
  }
}

async function createSession(env: Env, userId: string): Promise<string> {
  const token = 'glitchy_' + generateRandomHex(32);
  const now = Date.now();
  const expiresAt = now + 30 * 24 * 60 * 60 * 1000; // 30 days

  if (env.DB) {
    try {
      await env.DB.prepare(
        'INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)'
      )
        .bind(token, userId, now, expiresAt)
        .run();
    } catch (e) {
      console.error('D1 session insert error:', e);
    }
  }
  memSessions.set(token, { token, user_id: userId, created_at: now, expires_at: expiresAt });
  return token;
}

async function getSessionUser(env: Env, token: string): Promise<UserRecord | null> {
  const now = Date.now();
  if (env.DB) {
    try {
      const session = await env.DB.prepare(
        'SELECT * FROM sessions WHERE token = ?1 AND expires_at > ?2 LIMIT 1'
      )
        .bind(token, now)
        .first<SessionRecord>();
      if (session) {
        return await getUserById(env, session.user_id);
      }
    } catch (e) {
      console.error('D1 session query error:', e);
    }
  }

  const s = memSessions.get(token);
  if (s && s.expires_at > now) {
    return getUserById(env, s.user_id);
  }
  return null;
}

async function deleteSession(env: Env, token: string): Promise<void> {
  if (env.DB) {
    try {
      await env.DB.prepare('DELETE FROM sessions WHERE token = ?1').bind(token).run();
    } catch (e) {
      console.error('D1 delete session error:', e);
    }
  }
  memSessions.delete(token);
}

function userToDto(u: UserRecord) {
  return {
    id: u.id,
    username: u.username,
    email: u.email,
    role: u.role,
    badge: u.badge,
    skinData: u.skin_data || null,
    skinModel: u.skin_model || 'default',
    capeData: u.cape_data || null,
    createdAt: u.created_at,
  };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: CORS_HEADERS });
    }

    // 1. Health check
    if (url.pathname === '/health' || url.pathname === '/') {
      return Response.json(
        {
          status: 'ok',
          service: 'Glitchy Backend API',
          version: '1.3.1',
          dbReady: !!env.DB,
        },
        { headers: CORS_HEADERS }
      );
    }

    // 2. Register: POST /api/auth/register
    if (url.pathname === '/api/auth/register' && request.method === 'POST') {
      try {
        const body = (await request.json()) as { username?: string; email?: string; password?: string };
        const username = (body.username || '').trim();
        const email = (body.email || '').trim().toLowerCase();
        const password = body.password || '';

        // Validation
        if (!username || !/^[a-zA-Z0-9_]{3,16}$/.test(username)) {
          return Response.json(
            { success: false, error: 'نام کاربری باید بین ۳ تا ۱۶ کاراکتر و تنها شامل حروف انگلیسی، اعداد و زیرخط باشد.' },
            { status: 400, headers: CORS_HEADERS }
          );
        }

        if (!email || !isValidEmail(email)) {
          return Response.json(
            { success: false, error: 'لطفاً یک آدرس ایمیل معتبر وارد کنید.' },
            { status: 400, headers: CORS_HEADERS }
          );
        }

        if (isDisposableEmail(email)) {
          return Response.json(
            { success: false, error: 'استفاده از ایمیل‌های موقت (فیک) مجاز نمی‌باشد. لطفاً از ایمیل معتبر مانند Gmail یا Outlook استفاده کنید.' },
            { status: 400, headers: CORS_HEADERS }
          );
        }

        if (password.length < 6) {
          return Response.json(
            { success: false, error: 'رمز عبور باید حداقل ۶ کاراکتر باشد.' },
            { status: 400, headers: CORS_HEADERS }
          );
        }

        // Check if username already exists
        const existingUser = await getUserByUsernameOrEmail(env, username);
        if (existingUser && existingUser.username.toLowerCase() === username.toLowerCase()) {
          return Response.json(
            { success: false, error: 'این نام کاربری قبلاً ثبت شده است. لطفاً نام دیگری انتخاب کنید.' },
            { status: 409, headers: CORS_HEADERS }
          );
        }

        // Check if email already exists
        const existingEmail = await getUserByUsernameOrEmail(env, email);
        if (existingEmail && existingEmail.email.toLowerCase() === email.toLowerCase()) {
          return Response.json(
            { success: false, error: 'این ایمیل قبلاً ثبت شده است. لطفاً وارد شوید.' },
            { status: 409, headers: CORS_HEADERS }
          );
        }

        // Hash password
        const saltHex = generateRandomHex(16);
        const passHash = await hashPassword(password, saltHex);
        const now = Date.now();
        const userId = 'usr_' + generateRandomHex(12);

        const newUser: UserRecord = {
          id: userId,
          username,
          email,
          password_hash: passHash,
          salt: saltHex,
          role: 'user',
          badge: 'عضو گلیچی',
          skin_data: null,
          skin_model: 'default',
          cape_data: null,
          created_at: now,
          updated_at: now,
        };

        await insertUser(env, newUser);
        const token = await createSession(env, userId);

        return Response.json(
          {
            success: true,
            token,
            user: userToDto(newUser),
          },
          { headers: CORS_HEADERS }
        );
      } catch (err: any) {
        return Response.json(
          { success: false, error: 'خطای سرور: ' + (err?.message || 'نامشخص') },
          { status: 500, headers: CORS_HEADERS }
        );
      }
    }

    // 3. Login: POST /api/auth/login
    if (url.pathname === '/api/auth/login' && request.method === 'POST') {
      try {
        const body = (await request.json()) as { login?: string; password?: string };
        const login = (body.login || '').trim();
        const password = body.password || '';

        if (!login || !password) {
          return Response.json(
            { success: false, error: 'نام کاربری/ایمیل و رمز عبور الزامی است.' },
            { status: 400, headers: CORS_HEADERS }
          );
        }

        const user = await getUserByUsernameOrEmail(env, login);
        if (!user) {
          return Response.json(
            { success: false, error: 'حساب کاربری با این مشخصات یافت نشد.' },
            { status: 401, headers: CORS_HEADERS }
          );
        }

        const checkHash = await hashPassword(password, user.salt);
        if (checkHash !== user.password_hash) {
          return Response.json(
            { success: false, error: 'رمز عبور وارد شده نادرست است.' },
            { status: 401, headers: CORS_HEADERS }
          );
        }

        const token = await createSession(env, user.id);
        return Response.json(
          {
            success: true,
            token,
            user: userToDto(user),
          },
          { headers: CORS_HEADERS }
        );
      } catch (err: any) {
        return Response.json(
          { success: false, error: 'خطای سرور: ' + (err?.message || 'نامشخص') },
          { status: 500, headers: CORS_HEADERS }
        );
      }
    }

    // 4. Me: GET /api/auth/me
    if (url.pathname === '/api/auth/me' && request.method === 'GET') {
      const authHeader = request.headers.get('Authorization') || '';
      const token = authHeader.replace(/^Bearer\s+/i, '').trim();
      if (!token) {
        return Response.json({ success: false, error: 'عدم احراز هویت' }, { status: 401, headers: CORS_HEADERS });
      }

      const user = await getSessionUser(env, token);
      if (!user) {
        return Response.json({ success: false, error: 'نشست کاربری نامعتبر یا منقضی شده است' }, { status: 401, headers: CORS_HEADERS });
      }

      return Response.json({ success: true, user: userToDto(user) }, { headers: CORS_HEADERS });
    }

    // 5. Logout: POST /api/auth/logout
    if (url.pathname === '/api/auth/logout' && request.method === 'POST') {
      const authHeader = request.headers.get('Authorization') || '';
      const token = authHeader.replace(/^Bearer\s+/i, '').trim();
      if (token) {
        await deleteSession(env, token);
      }
      return Response.json({ success: true }, { headers: CORS_HEADERS });
    }

    // 6. Skin upload: POST /api/skins/upload
    if (url.pathname === '/api/skins/upload' && request.method === 'POST') {
      const authHeader = request.headers.get('Authorization') || '';
      const token = authHeader.replace(/^Bearer\s+/i, '').trim();
      if (!token) {
        return Response.json({ success: false, error: 'عدم احراز هویت' }, { status: 401, headers: CORS_HEADERS });
      }

      const user = await getSessionUser(env, token);
      if (!user) {
        return Response.json({ success: false, error: 'نشست نامعتبر است' }, { status: 401, headers: CORS_HEADERS });
      }

      try {
        const body = (await request.json()) as { skinData: string; model?: string; capeData?: string };
        if (!body.skinData) {
          return Response.json({ success: false, error: 'داده اسکین الزامی است' }, { status: 400, headers: CORS_HEADERS });
        }

        const model = body.model === 'slim' ? 'slim' : 'default';
        await updateUserSkin(env, user.id, body.skinData, model, body.capeData);

        return Response.json({ success: true }, { headers: CORS_HEADERS });
      } catch (err: any) {
        return Response.json({ success: false, error: 'خطای ذخیره اسکین: ' + err?.message }, { status: 500, headers: CORS_HEADERS });
      }
    }

    // 7. Skin public query: GET /api/skins/:username
    if (url.pathname.startsWith('/api/skins/') && request.method === 'GET') {
      const username = url.pathname.replace('/api/skins/', '').toLowerCase();
      const user = await getUserByUsernameOrEmail(env, username);
      if (!user || !user.skin_data) {
        return Response.json({ success: false, error: 'اسکین یافت نشد' }, { status: 404, headers: CORS_HEADERS });
      }

      return Response.json(
        {
          success: true,
          skin: {
            username: user.username,
            skinData: user.skin_data,
            model: user.skin_model,
            capeData: user.cape_data,
          },
        },
        { headers: CORS_HEADERS }
      );
    }

    // 8. Cloud sync: POST /api/sync & GET /api/sync
    if (url.pathname === '/api/sync') {
      const authHeader = request.headers.get('Authorization') || '';
      const token = authHeader.replace(/^Bearer\s+/i, '').trim();
      if (!token) {
        return Response.json({ success: false, error: 'عدم احراز هویت' }, { status: 401, headers: CORS_HEADERS });
      }

      const user = await getSessionUser(env, token);
      if (!user) {
        return Response.json({ success: false, error: 'نشست نامعتبر است' }, { status: 401, headers: CORS_HEADERS });
      }

      if (request.method === 'POST') {
        const body = (await request.json()) as { settingsJson?: string; instancesJson?: string };
        const now = Date.now();
        if (env.DB) {
          try {
            await env.DB.prepare(
              'INSERT INTO cloud_sync (user_id, settings_json, instances_json, updated_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(user_id) DO UPDATE SET settings_json = ?2, instances_json = ?3, updated_at = ?4'
            )
              .bind(user.id, body.settingsJson || null, body.instancesJson || null, now)
              .run();
          } catch (e) {
            console.error('D1 sync error:', e);
          }
        }
        memSync.set(user.id, { settings: body.settingsJson, instances: body.instancesJson, updatedAt: now });
        return Response.json({ success: true }, { headers: CORS_HEADERS });
      }

      if (request.method === 'GET') {
        if (env.DB) {
          try {
            const syncData = await env.DB.prepare('SELECT * FROM cloud_sync WHERE user_id = ?1 LIMIT 1')
              .bind(user.id)
              .first<{ settings_json?: string; instances_json?: string }>();
            if (syncData) {
              return Response.json(
                { success: true, settings: syncData.settings_json, instances: syncData.instances_json },
                { headers: CORS_HEADERS }
              );
            }
          } catch (e) {
            console.error('D1 sync get error:', e);
          }
        }
        const s = memSync.get(user.id);
        return Response.json(
          { success: true, settings: s?.settings, instances: s?.instances },
          { headers: CORS_HEADERS }
        );
      }
    }

    // 9. Chat Messages API (D1 Backed with Profanity Filter)
    if (url.pathname === '/api/chat/messages' && request.method === 'GET') {
      const channel = url.searchParams.get('channel') || 'global';
      const since = parseInt(url.searchParams.get('since') || '0', 10);
      if (env.DB) {
        try {
          const rows = await env.DB.prepare(
            'SELECT * FROM chat_messages WHERE channel_id = ?1 AND timestamp > ?2 ORDER BY timestamp ASC LIMIT 100'
          )
            .bind(channel, since)
            .all();
          const msgs = (rows.results || []).map((r: any) => ({
            id: r.id,
            channelId: r.channel_id,
            text: r.text,
            timestamp: r.timestamp,
            sender: {
              id: r.sender_id,
              username: r.sender_username,
              badge: r.sender_badge || 'عضو گلیچی',
              model: r.sender_model || 'default',
              avatarUrl: r.sender_avatar || undefined,
            },
          }));
          return Response.json({ success: true, messages: msgs }, { headers: CORS_HEADERS });
        } catch (e: any) {
          console.error('D1 chat get error:', e);
        }
      }
      return Response.json({ success: true, messages: [] }, { headers: CORS_HEADERS });
    }

    if (url.pathname === '/api/chat/messages' && request.method === 'POST') {
      const authHeader = request.headers.get('Authorization') || '';
      const token = authHeader.replace(/^Bearer\s+/i, '').trim();
      if (!token) {
        return Response.json({ success: false, error: 'عدم احراز هویت' }, { status: 401, headers: CORS_HEADERS });
      }
      const user = await getSessionUser(env, token);
      if (!user) {
        return Response.json({ success: false, error: 'نشست کاربری نامعتبر است' }, { status: 401, headers: CORS_HEADERS });
      }

      try {
        const body = (await request.json()) as { text: string; channelId?: string };
        const text = (body.text || '').trim();
        const channelId = body.channelId || 'global';

        if (!text) {
          return Response.json({ success: false, error: 'متن پیام خالی است' }, { status: 400, headers: CORS_HEADERS });
        }
        if (text.length > 500) {
          return Response.json({ success: false, error: 'پیام نباید بیش از ۵۰۰ کاراکتر باشد' }, { status: 400, headers: CORS_HEADERS });
        }

        // Anti-Profanity check
        if (containsProfanity(text)) {
          return Response.json(
            { success: false, error: 'پیام شما حاوی کلمات نامناسب است و ارسال نشد.' },
            { status: 400, headers: CORS_HEADERS }
          );
        }

        const msgId = `msg_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;
        const now = Date.now();

        if (env.DB) {
          await env.DB.prepare(
            'INSERT INTO chat_messages (id, channel_id, sender_id, sender_username, sender_badge, sender_model, sender_avatar, text, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)'
          )
            .bind(
              msgId,
              channelId,
              user.id,
              user.username,
              user.badge || 'عضو گلیچی',
              user.skin_model || 'default',
              user.skin_data || null,
              text,
              now
            )
            .run();
        }

        const createdMsg = {
          id: msgId,
          channelId,
          text,
          timestamp: now,
          sender: {
            id: user.id,
            username: user.username,
            badge: user.badge || 'عضو گلیچی',
            model: user.skin_model || 'default',
            avatarUrl: user.skin_data || undefined,
          },
        };

        return Response.json({ success: true, message: createdMsg }, { headers: CORS_HEADERS });
      } catch (err: any) {
        return Response.json({ success: false, error: 'خطا در ثبت پیام: ' + err?.message }, { status: 500, headers: CORS_HEADERS });
      }
    }

    // 10. Chat Groups API (Max 2 groups per user, D1 Backed)
    if (url.pathname === '/api/chat/groups' && request.method === 'GET') {
      if (env.DB) {
        try {
          const rows = await env.DB.prepare(
            'SELECT * FROM chat_groups WHERE visibility = "public" ORDER BY created_at DESC LIMIT 50'
          ).all();
          const groups = (rows.results || []).map((r: any) => ({
            id: r.id,
            name: r.name,
            description: r.description || '',
            ownerId: r.owner_id,
            inviteCode: r.invite_code,
            visibility: r.visibility,
            membersCount: r.members_count || 1,
            createdAt: r.created_at,
          }));
          return Response.json({ success: true, groups }, { headers: CORS_HEADERS });
        } catch (e: any) {
          console.error('D1 groups get error:', e);
        }
      }
      return Response.json({ success: true, groups: [] }, { headers: CORS_HEADERS });
    }

    if (url.pathname === '/api/chat/groups' && request.method === 'POST') {
      const authHeader = request.headers.get('Authorization') || '';
      const token = authHeader.replace(/^Bearer\s+/i, '').trim();
      if (!token) {
        return Response.json({ success: false, error: 'عدم احراز هویت' }, { status: 401, headers: CORS_HEADERS });
      }
      const user = await getSessionUser(env, token);
      if (!user) {
        return Response.json({ success: false, error: 'نشست کاربری نامعتبر است' }, { status: 401, headers: CORS_HEADERS });
      }

      try {
        const body = (await request.json()) as { name: string; description?: string; visibility?: string };
        const name = (body.name || '').trim();
        const description = (body.description || '').trim();
        const visibility = body.visibility === 'private' ? 'private' : 'public';

        if (!name || name.length < 2 || name.length > 30) {
          return Response.json({ success: false, error: 'نام گروه باید بین ۲ تا ۳۰ کاراکتر باشد' }, { status: 400, headers: CORS_HEADERS });
        }

        // Anti-Profanity check for group name and description
        if (containsProfanity(name) || containsProfanity(description)) {
          return Response.json(
            { success: false, error: 'نام یا توضیحات گروه حاوی کلمات نامناسب است.' },
            { status: 400, headers: CORS_HEADERS }
          );
        }

        if (env.DB) {
          // Enforce 2-group creation limit per user
          const countRow = await env.DB.prepare(
            'SELECT COUNT(*) as count FROM chat_groups WHERE owner_id = ?1'
          )
            .bind(user.id)
            .first<{ count: number }>();

          if (countRow && countRow.count >= 2) {
            return Response.json(
              { success: false, error: 'شما به سقف مجاز ساخت ۲ گروه رسیده‌اید.' },
              { status: 400, headers: CORS_HEADERS }
            );
          }

          const groupId = `grp_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;
          const inviteCode = 'GL-' + Math.random().toString(36).substring(2, 6).toUpperCase();
          const now = Date.now();

          await env.DB.prepare(
            'INSERT INTO chat_groups (id, name, description, owner_id, invite_code, visibility, members_count, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)'
          )
            .bind(groupId, name, description, user.id, inviteCode, visibility, now)
            .run();

          const group = {
            id: groupId,
            name,
            description,
            ownerId: user.id,
            inviteCode,
            visibility,
            membersCount: 1,
            createdAt: now,
          };

          return Response.json({ success: true, group }, { headers: CORS_HEADERS });
        }

        return Response.json({ success: false, error: 'خطای سرور دیتابیس' }, { status: 500, headers: CORS_HEADERS });
      } catch (err: any) {
        return Response.json({ success: false, error: 'خطا در ایجاد گروه: ' + err?.message }, { status: 500, headers: CORS_HEADERS });
      }
    }

    if (url.pathname === '/api/chat/groups/join' && request.method === 'POST') {
      try {
        const body = (await request.json()) as { inviteCode?: string; groupId?: string };
        const code = (body.inviteCode || '').trim().toUpperCase();
        const groupId = (body.groupId || '').trim();

        if (env.DB) {
          let group = null;
          if (code) {
            group = await env.DB.prepare('SELECT * FROM chat_groups WHERE invite_code = ?1 LIMIT 1')
              .bind(code)
              .first<any>();
          } else if (groupId) {
            group = await env.DB.prepare('SELECT * FROM chat_groups WHERE id = ?1 LIMIT 1')
              .bind(groupId)
              .first<any>();
          }

          if (!group) {
            return Response.json({ success: false, error: 'گروه یا کد دعوت یافت نشد' }, { status: 404, headers: CORS_HEADERS });
          }

          await env.DB.prepare('UPDATE chat_groups SET members_count = members_count + 1 WHERE id = ?1')
            .bind(group.id)
            .run();

          return Response.json(
            {
              success: true,
              group: {
                id: group.id,
                name: group.name,
                description: group.description || '',
                ownerId: group.owner_id,
                inviteCode: group.invite_code,
                visibility: group.visibility,
                membersCount: (group.members_count || 1) + 1,
                createdAt: group.created_at,
              },
            },
            { headers: CORS_HEADERS }
          );
        }
      } catch (err: any) {
        return Response.json({ success: false, error: 'خطا در عضویت: ' + err?.message }, { status: 500, headers: CORS_HEADERS });
      }
    }

    // 11. User public overview profile query
    if (url.pathname.startsWith('/api/users/') && request.method === 'GET') {
      const targetUsername = url.pathname.replace('/api/users/', '').trim();
      const targetUser = await getUserByUsernameOrEmail(env, targetUsername);
      if (!targetUser) {
        return Response.json({ success: false, error: 'کاربر یافت نشد' }, { status: 404, headers: CORS_HEADERS });
      }
      return Response.json(
        {
          success: true,
          user: userToDto(targetUser),
        },
        { headers: CORS_HEADERS }
      );
    }

    // 10. Presence API (legacy compatibility)
    if (url.pathname === '/api/presence') {
      if (request.method === 'GET') {
        const now = Date.now();
        const active: Record<string, unknown> = {};
        for (const [u, data] of userPresence.entries()) {
          if (now - data.updatedAt < 60_000) {
            active[u] = data;
          }
        }
        return Response.json({ success: true, presence: active }, { headers: CORS_HEADERS });
      }

      if (request.method === 'POST') {
        const body = (await request.json()) as { username: string; status: string; serverIp?: string; port?: number };
        if (body.username) {
          userPresence.set(body.username, {
            status: body.status,
            serverIp: body.serverIp,
            port: body.port,
            updatedAt: Date.now(),
          });
        }
        return Response.json({ success: true }, { headers: CORS_HEADERS });
      }
    }

    return Response.json({ error: 'Not found' }, { status: 404, headers: CORS_HEADERS });
  },
};
