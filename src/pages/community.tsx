import {
  Globe02Icon,
  LockIcon,
  PlusSignIcon,
  UserGroupIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import {
  Copy,
  Gamepad2,
  Hash,
  MessageSquare,
  Send,
  ShieldAlert,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  type CommunityGroup,
  CreateGroupModal,
} from "@/components/community/create-group-modal";
import {
  type CommunityUser,
  UserProfileModal,
} from "@/components/community/profile-modal";
import { MinecraftAvatar } from "@/components/minecraft-avatar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { backend } from "@/lib/utils";
import { useAccountStore } from "@/stores/account";
import { useConfig } from "@/stores/config";
import { useLocale } from "@/stores/locale";

const API_BASE = "https://glitchy-api.sepideh-help.workers.dev";

const PROFANITY_PATTERNS = [
  'کیر', 'کص', 'کسکش', 'کونکش', 'کونی', 'کون', 'جنده', 'مادرجنده', 'ننه جنده', 'دیوث', 'سکس', 'سکسی',
  'بیناموس', 'بی ناموس', 'حرومزاده', 'حرامزاده', 'لاشی', 'پدرسگ', 'پدر سگ', 'خواهرکسه', 'خارکسه', 'خارکسته',
  'خایه', 'خایه مال', 'ساک زدن', 'کسخول', 'کسخل', 'کوس', 'کوست', 'چوچول', 'شاش', 'عن', 'گوه',
  'مادرقحبه', 'قحبه', 'سیکتیر', 'سیک تیر', 'بکیرم', 'بکیر', 'بکصم', 'کسشر', 'کسشعر', 'کصشعر', 'کصشر',
  'kir', 'kos', 'koss', 'koon', 'jende', 'jendeh', 'dayoos', 'dayus', 'binamoos', 'haroomzade', 'lashi',
  'pedarsag', 'kharkose', 'khaye', 'shash', 'gooh', 'sik', 'siktir', 'koonkesh', 'koskesh',
  'fuck', 'fucking', 'bitch', 'asshole', 'dick', 'pussy', 'whore', 'slut', 'cunt', 'nigger', 'nigga'
];

function hasProfanity(text: string): boolean {
  if (!text) return false;
  const raw = text.toLowerCase();
  let normalized = raw
    .replace(/[يك]/g, (c) => (c === 'ي' ? 'ی' : 'ک'))
    .replace(/[\u200B-\u200D\uFEFF]/g, '')
    .replace(/[._\-*#@!+=~`|\\/:;,?^%$()[\]{}<>"]/g, '');
  const collapsed = normalized.replace(/(.)\1{2,}/g, '$1$1');
  for (const p of PROFANITY_PATTERNS) {
    if (normalized.includes(p) || collapsed.includes(p)) return true;
  }
  const words = raw.split(/\s+/);
  for (const w of words) {
    const clean = w.replace(/[^\p{L}\p{N}]/gu, '');
    for (const p of PROFANITY_PATTERNS) {
      if (clean === p) return true;
    }
  }
  return false;
}

interface ChatMessage {
  channelId: string;
  id: string;
  sender: CommunityUser;
  text: string;
  timestamp: number;
}

// Initial pre-seeded channels
const DEFAULT_CHANNELS = [
  {
    desc: "Main public chat for all Glitchy users",
    id: "global",
    name: "Global",
  },
  { desc: "Ask questions, fix crashes", id: "help", name: "Help & Support" },
  {
    desc: "Share worlds and shaders",
    id: "modpacks",
    name: "Modpacks & Builds",
  },
];

const PRE_SEEDED_USERS: Record<string, CommunityUser> = {
  alex: {
    bio: "Building medieval castles in survival mode!",
    currentServerIp: "play.glitchy.ir",
    currentServerName: "Glitchy Survival SMP",
    currentServerPort: 25_565,
    gameVersion: "1.21.4",
    id: "user_alex",
    memberSince: "May 2026",
    model: "slim",
    status: "in_game",
    username: "Alex_Builder",
  },
  mina: {
    bio: "Speedrunner & PvP player.",
    currentServerIp: "mc.hypixel.net",
    currentServerName: "Hypixel Network",
    currentServerPort: 25_565,
    gameVersion: "1.21.1",
    id: "user_mina",
    memberSince: "Feb 2026",
    model: "slim",
    status: "in_game",
    username: "MinaCraft",
  },
  steve: {
    bio: "Redstone fanatic & technical player.",
    id: "user_steve",
    memberSince: "Jan 2026",
    model: "default",
    status: "online",
    username: "RedstonePro",
  },
};

const INITIAL_MESSAGES: ChatMessage[] = [
  {
    channelId: "global",
    id: "msg_1",
    sender: PRE_SEEDED_USERS.alex,
    text: "Hey everyone! Welcome to Glitchy Community chat!",
    timestamp: Date.now() - 1000 * 60 * 12,
  },
  {
    channelId: "global",
    id: "msg_2",
    sender: PRE_SEEDED_USERS.mina,
    text: "Anyone down for some Bedwars or Survival? Click my profile to join my server!",
    timestamp: Date.now() - 1000 * 60 * 5,
  },
  {
    channelId: "global",
    id: "msg_3",
    sender: PRE_SEEDED_USERS.steve,
    text: "The new 3D skins look amazing in multiplayer 🔥",
    timestamp: Date.now() - 1000 * 60 * 1,
  },
];

const SPAM_MAX_BURST = 5;
const SPAM_WINDOW_MS = 5000;
const SPAM_MUTE_SECONDS = 300; // 5 minutes

export default function Community() {
  const { locale } = useLocale();
  const isFa = locale === "fa";

  const { user, openAuthModal } = useAccountStore();
  const currentUsername = user?.username || "GlitchyPlayer";

  const [activeChannelId, setActiveChannelId] = useState<string>("global");
  const [messages, setMessages] = useState<ChatMessage[]>(() => {
    try {
      const saved = localStorage.getItem("glitchy_chat_messages_v1");
      if (saved) {
        return JSON.parse(saved);
      }
    } catch {
      // ignore
    }
    return INITIAL_MESSAGES;
  });

  const [inputText, setInputText] = useState("");
  const [activeUserModal, setActiveUserModal] = useState<CommunityUser | null>(
    null
  );
  const [isCreateGroupOpen, setIsCreateGroupOpen] = useState(false);
  const [groups, setGroups] = useState<CommunityGroup[]>(() => {
    try {
      const saved = localStorage.getItem("glitchy_custom_groups_v1");
      if (saved) {
        return JSON.parse(saved);
      }
    } catch {
      // ignore
    }
    return [];
  });

  // Anti-Spam state
  const messageTimestampsRef = useRef<number[]>([]);
  const [mutedUntil, setMutedUntil] = useState<number>(0);
  const [remainingCooldown, setRemainingCooldown] = useState<number>(0);

  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Sync messages
  useEffect(() => {
    try {
      localStorage.setItem(
        "glitchy_chat_messages_v1",
        JSON.stringify(messages)
      );
    } catch {
      // ignore
    }
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Sync groups
  useEffect(() => {
    try {
      localStorage.setItem("glitchy_custom_groups_v1", JSON.stringify(groups));
    } catch {
      // ignore
    }
  }, [groups]);

  // Anti-Spam Countdown timer
  useEffect(() => {
    if (mutedUntil <= Date.now()) {
      setRemainingCooldown(0);
      return;
    }

    const interval = setInterval(() => {
      const diff = Math.ceil((mutedUntil - Date.now()) / 1000);
      if (diff <= 0) {
        setMutedUntil(0);
        setRemainingCooldown(0);
        clearInterval(interval);
      } else {
        setRemainingCooldown(diff);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [mutedUntil]);

  // Fetch messages from Cloudflare Worker D1
  useEffect(() => {
    let isMounted = true;

    const fetchMessages = async () => {
      try {
        const res = await fetch(
          `${API_BASE}/api/chat/messages?channel=${encodeURIComponent(activeChannelId)}`
        );
        const data = await res.json();
        if (data.success && Array.isArray(data.messages) && isMounted) {
          setMessages((prev) => {
            const others = prev.filter((m) => m.channelId !== activeChannelId);
            const existingMap = new Map(
              prev
                .filter((m) => m.channelId === activeChannelId)
                .map((m) => [m.id, m])
            );
            for (const m of data.messages) {
              existingMap.set(m.id, m);
            }
            const currentChannel = Array.from(existingMap.values());
            return [...others, ...currentChannel].sort(
              (a, b) => a.timestamp - b.timestamp
            );
          });
        }
      } catch {
        // Network error ignored in poller
      }
    };

    fetchMessages();
    const interval = setInterval(fetchMessages, 2500);

    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, [activeChannelId]);

  // Fetch public community groups from Cloudflare Worker D1
  useEffect(() => {
    let isMounted = true;

    const fetchGroups = async () => {
      try {
        const res = await fetch(`${API_BASE}/api/chat/groups`);
        const data = await res.json();
        if (data.success && Array.isArray(data.groups) && isMounted) {
          setGroups((prev) => {
            const map = new Map(prev.map((g) => [g.id, g]));
            for (const g of data.groups) {
              map.set(g.id, g);
            }
            return Array.from(map.values());
          });
        }
      } catch {
        // Network error ignored in poller
      }
    };

    fetchGroups();
    const interval = setInterval(fetchGroups, 10_000);

    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, []);

  const activeChannelName =
    DEFAULT_CHANNELS.find((c) => c.id === activeChannelId)?.name ||
    groups.find((g) => g.id === activeChannelId)?.name ||
    "Global";

  const activeChannelDesc =
    DEFAULT_CHANNELS.find((c) => c.id === activeChannelId)?.desc ||
    groups.find((g) => g.id === activeChannelId)?.description ||
    "";

  const userCreatedCount = groups.filter(
    (g) => g.ownerId === (user?.id || "current_user")
  ).length;

  // Handle Send Message with Anti-Spam & Anti-Profanity enforcement
  const handleSendMessage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputText.trim()) {
      return;
    }

    if (!user) {
      toast.error(
        isFa
          ? "برای ارسال پیام ابتدا وارد حساب گلیچی شوید."
          : "Please login to Glitchy Account first."
      );
      openAuthModal("login");
      return;
    }

    // Check Mute status
    const now = Date.now();
    if (mutedUntil > now) {
      toast.error(
        isFa
          ? "سیستم ضداسپم: شما موقتاً از ارسال پیام محروم هستید."
          : "Anti-spam active. You are temporarily muted."
      );
      return;
    }

    // Anti-Profanity Filter
    if (hasProfanity(inputText)) {
      toast.error(
        isFa
          ? "پیام شما حاوی کلمات نامناسب است و ارسال نشد."
          : "Your message contains profanity and was blocked."
      );
      return;
    }

    // Check burst rate
    const recent = messageTimestampsRef.current.filter(
      (ts) => now - ts < SPAM_WINDOW_MS
    );
    recent.push(now);
    messageTimestampsRef.current = recent;

    if (recent.length >= SPAM_MAX_BURST) {
      const muteExpiry = now + SPAM_MUTE_SECONDS * 1000;
      setMutedUntil(muteExpiry);
      setRemainingCooldown(SPAM_MUTE_SECONDS);
      toast.error(
        isFa
          ? "کمی آهسته‌تر! شما به مدت ۵ دقیقه از چت کردن محروم شدید."
          : "Slow down! You sent messages too quickly. Muted for 5 minutes."
      );
      return;
    }

    const textToSend = inputText.trim();
    setInputText("");

    try {
      const token = await backend("glitchy_account_get_token");
      const res = await fetch(`${API_BASE}/api/chat/messages`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token || ""}`,
        },
        body: JSON.stringify({
          channelId: activeChannelId,
          text: textToSend,
        }),
      });
      const data = await res.json();
      if (data.success && data.message) {
        setMessages((prev) => {
          if (prev.some((m) => m.id === data.message.id)) return prev;
          return [...prev, data.message];
        });
      } else {
        toast.error(
          data.error ||
            (isFa ? "خطا در ارسال پیام" : "Failed to send message")
        );
      }
    } catch {
      toast.error(
        isFa
          ? "خطا در برقراری ارتباط با سرور چت"
          : "Failed to connect to chat server"
      );
    }
  };

  // Create Group Handler (Max 2 groups per user)
  const handleCreateGroup = async (
    groupData: Omit<
      CommunityGroup,
      "id" | "inviteCode" | "createdAt" | "membersCount"
    >
  ) => {
    if (!user) {
      toast.error(
        isFa
          ? "برای ساخت گروه ابتدا وارد حساب گلیچی شوید."
          : "Please login to Glitchy Account first."
      );
      openAuthModal("login");
      return;
    }

    if (userCreatedCount >= 2) {
      toast.error(
        isFa
          ? "شما به سقف مجاز ۲ گروه ایجاد شده رسیده‌اید."
          : "You have reached the maximum limit of 2 created communities."
      );
      return;
    }

    if (hasProfanity(groupData.name) || hasProfanity(groupData.description)) {
      toast.error(
        isFa
          ? "نام یا توضیحات گروه حاوی کلمات نامناسب است."
          : "Group name or description contains profanity."
      );
      return;
    }

    try {
      const token = await backend("glitchy_account_get_token");
      const res = await fetch(`${API_BASE}/api/chat/groups`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token || ""}`,
        },
        body: JSON.stringify(groupData),
      });
      const data = await res.json();
      if (data.success && data.group) {
        setGroups((prev) => {
          if (prev.some((g) => g.id === data.group.id)) return prev;
          return [...prev, data.group];
        });
        setActiveChannelId(data.group.id);
        toast.success(
          isFa
            ? `گروه ${data.group.name} با موفقیت ساخته شد!`
            : `Community ${data.group.name} created!`
        );
      } else {
        toast.error(
          data.error ||
            (isFa ? "خطا در ساخت گروه" : "Failed to create group")
        );
      }
    } catch {
      toast.error(
        isFa ? "خطا در برقراری ارتباط با سرور" : "Failed to connect to server"
      );
    }
  };

  // Copy Invite Link
  const handleCopyInviteLink = (group: CommunityGroup) => {
    const link = `glitchy://community/join/${group.inviteCode}`;
    navigator.clipboard.writeText(link);
    toast.success(
      isFa
        ? `لینک دعوت گروه کپی شد: ${link}`
        : `Invite link copied to clipboard: ${link}`
    );
  };

  // Quick Direct Server Join Handler
  const handleJoinServer = async (serverIp: string, port = 25_565) => {
    setActiveUserModal(null);
    toast.info(
      isFa
        ? `در حال اجرای ماینکرفت و ورود مستقیم به ${serverIp}:${port}...`
        : `Launching Minecraft and connecting directly to ${serverIp}:${port}...`
    );
    try {
      const activeVersion = useConfig.getState().version || "1.21.4";
      await invoke("play", {
        directConnect: { port, server: serverIp },
        selectedVersion: activeVersion,
      });
    } catch {
      // Fallback
    }
  };

  const channelMessages = messages.filter(
    (m) => m.channelId === activeChannelId
  );

  return (
    <div className="flex h-full min-h-0 gap-4 overflow-hidden">
      {/* Left Sidebar: Channels & Communities */}
      <aside className="flex w-64 shrink-0 flex-col rounded-2xl border border-border/60 bg-secondary/15 p-3 backdrop-blur-md">
        <div className="mb-3 flex items-center justify-between px-2 pt-1">
          <div className="flex items-center gap-2">
            <HugeiconsIcon
              className="text-primary"
              icon={UserGroupIcon}
              size={18}
              strokeWidth={2}
            />
            <h2 className="font-bold text-foreground text-sm">Community</h2>
          </div>
          <Button
            className="size-7 rounded-lg p-0"
            onClick={() => setIsCreateGroupOpen(true)}
            size="sm"
            title="Create Community"
            variant="ghost"
          >
            <HugeiconsIcon icon={PlusSignIcon} size={16} />
          </Button>
        </div>

        <div className="scrollbar-thin flex-1 space-y-4 overflow-y-auto pr-1">
          {/* Default Channels */}
          <div>
            <div className="px-2 pb-1.5 font-bold text-[10px] text-muted-foreground uppercase tracking-wider">
              Main Channels
            </div>
            <div className="space-y-1">
              {DEFAULT_CHANNELS.map((ch) => {
                const isActive = activeChannelId === ch.id;
                return (
                  <button
                    className={`flex w-full items-center gap-2 rounded-xl px-3 py-2 font-semibold text-xs transition-all ${
                      isActive
                        ? "bg-primary text-primary-foreground shadow-xs"
                        : "text-muted-foreground hover:bg-secondary/40 hover:text-foreground"
                    }`}
                    key={ch.id}
                    onClick={() => setActiveChannelId(ch.id)}
                    type="button"
                  >
                    <Hash className="size-4 shrink-0" />
                    <span className="truncate">{ch.name}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* User-Created Groups */}
          <div>
            <div className="flex items-center justify-between px-2 pb-1.5">
              <span className="font-bold text-[10px] text-muted-foreground uppercase tracking-wider">
                My Communities ({userCreatedCount}/2)
              </span>
            </div>
            <div className="space-y-1">
              {groups.map((group) => {
                const isActive = activeChannelId === group.id;
                return (
                  <div
                    className={`group flex items-center justify-between rounded-xl px-3 py-2 font-semibold text-xs transition-all ${
                      isActive
                        ? "bg-primary text-primary-foreground shadow-xs"
                        : "text-muted-foreground hover:bg-secondary/40 hover:text-foreground"
                    }`}
                    key={group.id}
                  >
                    <button
                      className="flex flex-1 items-center gap-2 truncate text-start"
                      onClick={() => setActiveChannelId(group.id)}
                      type="button"
                    >
                      {group.visibility === "private" ? (
                        <HugeiconsIcon icon={LockIcon} size={14} />
                      ) : (
                        <HugeiconsIcon icon={Globe02Icon} size={14} />
                      )}
                      <span className="truncate">{group.name}</span>
                    </button>
                    {group.visibility === "private" && (
                      <button
                        className="p-1 opacity-0 transition-opacity hover:text-white group-hover:opacity-100"
                        onClick={() => handleCopyInviteLink(group)}
                        title="Copy Invite Link"
                        type="button"
                      >
                        <Copy className="size-3" />
                      </button>
                    )}
                  </div>
                );
              })}

              {groups.length === 0 && (
                <div className="p-3 text-center text-[11px] text-muted-foreground">
                  No groups yet. Click + to create one.
                </div>
              )}
            </div>
          </div>
        </div>
      </aside>

      {/* Main Chat Stream Area */}
      <main className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border/60 bg-secondary/10 backdrop-blur-md">
        {/* Chat Header */}
        <header className="flex h-14 shrink-0 items-center justify-between border-border/50 border-b bg-background/30 px-5">
          <div className="flex items-center gap-2.5">
            <Hash className="size-5 text-primary" />
            <div>
              <h3 className="font-bold text-foreground text-sm">
                {activeChannelName}
              </h3>
              <p className="truncate text-[11px] text-muted-foreground">
                {activeChannelDesc}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1.5 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-0.5 font-semibold text-[11px] text-emerald-400">
              <span className="size-1.5 animate-pulse rounded-full bg-emerald-400" />
              Online Hub
            </span>
          </div>
        </header>

        {/* Messages Feed */}
        <div className="scrollbar-thin flex-1 space-y-4 overflow-y-auto p-5">
          {channelMessages.map((msg) => (
            <div
              className="group flex items-start gap-3 text-start"
              key={msg.id}
            >
              {/* Avatar Icon / Clickable */}
              <button
                className="size-9 shrink-0 overflow-hidden rounded-xl border border-border/50 bg-background/60 transition-colors hover:border-primary/60"
                onClick={() => setActiveUserModal(msg.sender)}
                type="button"
              >
                <MinecraftAvatar
                  size={36}
                  skinUrl={msg.sender.skinUrl}
                  username={msg.sender.username}
                />
              </button>

              <div className="min-w-0 flex-1 space-y-1">
                <div className="flex items-center gap-2">
                  <button
                    className="truncate font-bold text-foreground text-xs transition-colors hover:text-primary"
                    onClick={() => setActiveUserModal(msg.sender)}
                    type="button"
                  >
                    {msg.sender.username}
                  </button>

                  {/* Status Indicator badge */}
                  {msg.sender.status === "in_game" && (
                    <span
                      className="inline-flex cursor-pointer items-center gap-1 rounded-md border border-emerald-500/30 bg-emerald-500/15 px-1.5 py-0.2 font-semibold text-[10px] text-emerald-400 transition-colors hover:bg-emerald-500/25"
                      onClick={() => setActiveUserModal(msg.sender)}
                    >
                      <Gamepad2 className="size-3" />
                      <span>{msg.sender.currentServerName || "In Game"}</span>
                    </span>
                  )}

                  <span className="text-[10px] text-muted-foreground">
                    {new Date(msg.timestamp).toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                </div>

                <p className="break-words text-foreground/90 text-xs leading-relaxed">
                  {msg.text}
                </p>
              </div>
            </div>
          ))}

          {channelMessages.length === 0 && (
            <div className="flex h-full flex-col items-center justify-center space-y-2 p-8 text-center">
              <MessageSquare className="size-8 text-muted-foreground/40" />
              <p className="font-semibold text-muted-foreground text-xs">
                {isFa
                  ? "هنوز پیامی در این کانال نیست. اولین پیام را ارسال کنید!"
                  : "No messages in this channel yet. Be the first to say hello!"}
              </p>
            </div>
          )}

          <div ref={messagesEndRef} />
        </div>

        {/* Input Bar & Anti-Spam Indicator */}
        <div className="border-border/50 border-t bg-background/30 p-3.5">
          {remainingCooldown > 0 ? (
            <div className="flex animate-pulse items-center justify-center gap-2 rounded-xl border border-destructive/40 bg-destructive/10 p-2.5 font-semibold text-destructive text-xs">
              <ShieldAlert className="size-4 shrink-0" />
              <span>
                {isFa
                  ? `سیستم ضداسپم فعال است. ارسال پیام تا ${remainingCooldown} ثانیه دیگر مسدود است.`
                  : `Anti-spam cooldown active. You can chat again in ${Math.floor(remainingCooldown / 60)}m ${remainingCooldown % 60}s.`}
              </span>
            </div>
          ) : (
            <form
              className="flex items-center gap-2"
              onSubmit={handleSendMessage}
            >
              <Input
                className="h-10 border-border/50 bg-background/50 text-xs"
                maxLength={300}
                onChange={(e) => setInputText(e.target.value)}
                placeholder={`${isFa ? "ارسال پیام در" : "Message in"} #${activeChannelName}...`}
                value={inputText}
              />
              <Button
                className="size-10 shrink-0 rounded-xl p-0"
                disabled={!inputText.trim()}
                type="submit"
              >
                <Send className="size-4" />
              </Button>
            </form>
          )}
        </div>
      </main>

      {/* Profile Popup Modal on click */}
      <UserProfileModal
        onClose={() => setActiveUserModal(null)}
        onJoinServer={handleJoinServer}
        user={activeUserModal}
      />

      {/* Create Group Modal */}
      <CreateGroupModal
        isOpen={isCreateGroupOpen}
        onClose={() => setIsCreateGroupOpen(false)}
        onCreateGroup={handleCreateGroup}
        userCreatedCount={userCreatedCount}
      />
    </div>
  );
}
