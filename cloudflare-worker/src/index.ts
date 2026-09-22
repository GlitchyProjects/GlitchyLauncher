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

    // 9. Chat messages API (legacy compatibility)
    if (url.pathname === '/api/chat/messages') {
      if (request.method === 'GET') {
        const channel = url.searchParams.get('channel') || 'global';
        const filtered = messages.filter((m) => m.channelId === channel).slice(-100);
        return Response.json({ success: true, messages: filtered }, { headers: CORS_HEADERS });
      }

      if (request.method === 'POST') {
        try {
          const body = (await request.json()) as any;
          if (!body.text || !body.sender?.username) {
            return Response.json({ success: false, error: 'Invalid payload' }, { status: 400, headers: CORS_HEADERS });
          }
          body.timestamp = Date.now();
          body.id = msg__;
          messages.push(body);
          if (messages.length > 500) messages.shift();
          return Response.json({ success: true, message: body }, { headers: CORS_HEADERS });
        } catch {
          return Response.json({ success: false, error: 'Invalid JSON' }, { status: 400, headers: CORS_HEADERS });
        }
      }
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
