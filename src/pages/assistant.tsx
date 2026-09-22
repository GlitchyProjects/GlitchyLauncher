import {
  CheckmarkCircle02Icon,
  CpuIcon,
  InformationCircleIcon,
  Message01Icon,
  RefreshIcon,
  RepairIcon,
  SentIcon,
  Shield01Icon,
  SparklesIcon,
  ViewIcon,
  Wrench01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type { AiDiagnosisResult, ChatMessage } from "@/invokes";
import { cn } from "@/lib/utils";
import { useConfig } from "@/stores/config";

import { useLocale } from "@/stores/locale";

const isFaLocale = () => useLocale.getState().locale === "fa";

const SUGGESTED_PROMPTS_EN = [
  "How do I install Sodium in the launcher?",
  "What are the best graphic settings for my system?",
  "How to install shaders or modpacks in Glitchy?",
  "Why is my Minecraft crashing with OutOfMemory?",
];

const SUGGESTED_PROMPTS_FA = [
  "چطور سودیوم (Sodium) رو در لانچر نصب کنم؟",
  "بهترین تنظیمات گرافیک برای سیستم من چیه؟",
  "چطور شیدر یا مادپک در لانچر نصب کنم؟",
  "چرا ماینکرفت من ارور OutOfMemory میده؟",
];

const CHAT_STORAGE_KEY = "glitchy_ai_chat_history";

const getInitialChatMessage = (): ChatMessage => ({
  content: isFaLocale()
    ? "سلام گیمر عزیز! 👋 من دستیار هوش مصنوعی Glitchy Launcher هستم.\nهر سوالی درباره ماینکرفت، رفع ارورها و کرش‌ها، افزایش FPS یا تنظیمات بازی داری بپرس تا کمکت کنم!"
    : "Hello! 👋 I am your Glitchy Launcher AI Assistant.\nAsk me anything about Minecraft, fixing errors and crashes, boosting FPS, or configuring game settings!",
  role: "assistant",
});

// Helper to detect if text contains Persian / Arabic characters
function containsRtl(text: string): boolean {
  return /[\u0600-\u06FF\u0750-\u077F\uFB50-\uFDFF\uFE70-\uFEFF]/.test(text);
}

// Helper to isolate English technical terms/mod names/versions inside Persian text to avoid BiDi scrambling
function BidiInlineTokens({ text, isRtl }: { text: string; isRtl: boolean }) {
  if (!isRtl) {
    return <span>{text}</span>;
  }

  // Group Latin words, mod names, numbers, paths, commands (e.g. "Fabric API", "Sodium 0.5.8", "Java 21", "Exit Code 1")
  const latinRegex = /([A-Za-z0-9_./\\-]+(?:\s+[A-Za-z0-9_./\\-]+)*)/g;
  const tokens = text.split(latinRegex);

  return (
    <>
      {tokens.map((token, idx) => {
        if (/[A-Za-z0-9]/.test(token)) {
          return (
            <bdi
              className="mx-0.5 inline font-medium text-foreground/95"
              dir="ltr"
              key={`bdi-${token}-${idx}`}
            >
              {token}
            </bdi>
          );
        }
        return <span key={`text-${token}-${idx}`}>{token}</span>;
      })}
    </>
  );
}

// Line renderer that isolates code blocks, markdown bold, and English terms
function BidiLine({ text }: { text: string }) {
  const trimmed = text.trim();
  if (!trimmed) {
    return <div className="h-2" />;
  }

  const isRtl = containsRtl(text);
  const codeParts = text.split(/(`[^`]+`)/g);

  return (
    <div
      className={cn(
        "my-0.5 min-h-[1.25rem] leading-relaxed",
        isRtl ? "text-right font-sans" : "text-left font-sans"
      )}
      dir={isRtl ? "rtl" : "ltr"}
      style={{ unicodeBidi: "isolate" }}
    >
      {codeParts.map((part, partIdx) => {
        if (part.startsWith("`") && part.endsWith("`") && part.length >= 2) {
          const codeContent = part.slice(1, -1);
          return (
            <code
              className="mx-1 inline-block select-text rounded border border-border/50 bg-background/80 px-1.5 py-0.5 align-baseline font-mono font-semibold text-[11px] text-primary"
              dir="ltr"
              key={`code-${partIdx}-${codeContent.slice(0, 8)}`}
            >
              <bdi dir="ltr">{codeContent}</bdi>
            </code>
          );
        }

        const boldParts = part.split(/(\*\*[^*]+\*\*)/g);
        return boldParts.map((bPart, bIdx) => {
          if (
            bPart.startsWith("**") &&
            bPart.endsWith("**") &&
            bPart.length >= 4
          ) {
            const boldContent = bPart.slice(2, -2);
            return (
              <strong
                className="font-bold text-foreground"
                key={`bold-${bIdx}-${boldContent.slice(0, 8)}`}
              >
                <BidiInlineTokens isRtl={isRtl} text={boldContent} />
              </strong>
            );
          }
          return (
            <BidiInlineTokens
              isRtl={isRtl}
              key={`raw-${bIdx}-${bPart.slice(0, 8)}`}
              text={bPart}
            />
          );
        });
      })}
    </div>
  );
}

export function BidiMessageView({
  content,
  className,
}: {
  content: string;
  className?: string;
}) {
  const lines = content.split("\n");
  return (
    <div className={cn("select-text space-y-0.5", className)}>
      {lines.map((line, idx) => (
        <BidiLine key={`line-${idx}-${line.slice(0, 10)}`} text={line} />
      ))}
    </div>
  );
}

export default function AssistantPage() {
  const [activeTab, setActiveTab] = useState("diagnostic");
  const { version: currentVersion } = useConfig();

  // --- Consent & Privacy State ---
  const [shareDiagnostics, setShareDiagnostics] = useState<boolean>(
    () => localStorage.getItem("glitchy_ai_share_diagnostics") !== "false"
  );

  const handleToggleDiagnostics = (checked: boolean) => {
    setShareDiagnostics(checked);
    localStorage.setItem("glitchy_ai_share_diagnostics", String(checked));
    if (checked) {
      toast.success("System & game diagnostics sharing enabled.");
    } else {
      toast.info(
        "Diagnostics sharing disabled. AI will only see your text messages."
      );
    }
  };

  // Fetch live system diagnostics
  const {
    data: sysData,
    isLoading: isSysDataLoading,
    refetch: refetchSysData,
  } = useBackend({
    name: "get_system_diagnostics",
  });

  // Build formatted context string for AI
  const formattedSystemContext = useMemo(() => {
    if (!(shareDiagnostics && sysData)) {
      return;
    }
    const modsList =
      sysData.installedMods.length > 0
        ? sysData.installedMods.slice(0, 40).join(", ") +
          (sysData.installedMods.length > 40
            ? ` ...and ${sysData.installedMods.length - 40} more`
            : "")
        : "None";

    return [
      `OS: ${sysData.os}`,
      `CPU: ${sysData.cpu}`,
      `Total RAM: ${sysData.totalRamMb} MB (Free: ${sysData.freeRamMb} MB)`,
      `Allocated Game RAM: ${sysData.allocatedRamMb} MB`,
      `GPU: ${sysData.gpu}`,
      `Java Runtime: ${sysData.selectedJava}`,
      `Selected Minecraft Version: ${currentVersion ?? sysData.gameVersion ?? "1.20.1"}`,
      `Installed Mods (${sysData.installedMods.length}): ${modsList}`,
    ].join("\n");
  }, [shareDiagnostics, sysData, currentVersion]);

  // --- Diagnostic State ---
  const [manualLog, setManualLog] = useState("");
  const [diagnosis, setDiagnosis] = useState<AiDiagnosisResult | null>(null);
  const [isDiagnosing, setIsDiagnosing] = useState(false);
  const [isFixing, setIsFixing] = useState(false);

  // --- Chat State ---
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>(() => {
    try {
      const saved = localStorage.getItem(CHAT_STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (Array.isArray(parsed) && parsed.length > 0) {
          return parsed;
        }
      }
    } catch {
      // ignore parse error
    }
    return [getInitialChatMessage()];
  });
  const [chatInput, setChatInput] = useState("");
  const [isChatLoading, setIsChatLoading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Sync chat messages to localStorage so leaving the tab doesn't lose conversation
  useEffect(() => {
    try {
      localStorage.setItem(CHAT_STORAGE_KEY, JSON.stringify(chatMessages));
    } catch {
      // ignore storage error
    }
  }, [chatMessages]);

  const handleResetChat = () => {
    setChatMessages([getInitialChatMessage()]);
    try {
      localStorage.removeItem(CHAT_STORAGE_KEY);
    } catch {
      // ignore
    }
    toast.success(
      isFaLocale() ? "گفتگو با موفقیت بازنشانی شد." : "Chat reset successfully."
    );
  };

  const { mutateAsync: diagnoseMutation } = useBackendMutation({
    name: "ai_diagnose_log",
  });
  const { mutateAsync: fixMutation } = useBackendMutation({
    name: "ai_apply_autofix",
  });
  const { mutateAsync: chatMutation } = useBackendMutation({
    name: "ai_chat",
  });

  // Fetch live AI rate limit / usage status (20 requests per 5 hours)
  const { data: usageStatus, refetch: refetchUsageStatus } = useBackend({
    name: "get_ai_usage_status",
  });

  const formatResetTime = (seconds?: number) => {
    if (!seconds || seconds <= 0) {
      return isFaLocale() ? "به زودی" : "Soon";
    }
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    if (hours > 0) {
      return isFaLocale()
        ? `${hours} ساعت و ${minutes} دقیقه`
        : `${hours}h ${minutes}m`;
    }
    return isFaLocale()
      ? `${Math.max(1, minutes)} دقیقه`
      : `${Math.max(1, minutes)}m`;
  };

  // Auto-scroll chat to bottom on new messages
  useEffect(() => {
    if (activeTab === "chat") {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [chatMessages, isChatLoading, activeTab]);

  // Handle Log Diagnosis
  const handleDiagnose = async (useManual = false) => {
    if (usageStatus?.remainingRequests === 0) {
      toast.error(
        `سقف مجاز ۲۰ درخواست در ۵ ساعت تکمیل شده است. لطفاً ${formatResetTime(usageStatus.resetInSeconds)} دیگر تلاش کنید.`
      );
      return;
    }
    setIsDiagnosing(true);
    try {
      const result = await diagnoseMutation({
        manualLog: useManual && manualLog.trim() ? manualLog.trim() : undefined,
        systemContext: formattedSystemContext,
      });
      setDiagnosis(result);
      toast.success("AI crash analysis completed successfully.");
    } catch (err: unknown) {
      const errorMsg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : String(err);
      toast.error(errorMsg || "Failed to analyze game crash log.");
    } finally {
      setIsDiagnosing(false);
      refetchUsageStatus();
    }
  };

  // Handle Auto-Fix
  const handleAutoFix = async () => {
    if (!diagnosis?.detectedIssueType) {
      return;
    }
    setIsFixing(true);
    try {
      const response = await fixMutation({
        action: diagnosis.detectedIssueType,
      });
      toast.success(response);
      setDiagnosis((prev) =>
        prev ? { ...prev, autoFixAvailable: false } : null
      );
    } catch (err: unknown) {
      const errorMsg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : String(err);
      toast.error(errorMsg || "Failed to apply automatic fix.");
    } finally {
      setIsFixing(false);
    }
  };

  // Handle Chat Send
  const handleSendMessage = async (textToSend?: string) => {
    const text = (textToSend ?? chatInput).trim();
    if (!text || isChatLoading) {
      return;
    }

    if (usageStatus?.remainingRequests === 0) {
      toast.error(
        `سقف مجاز ۲۰ درخواست در ۵ ساعت تکمیل شده است. لطفاً ${formatResetTime(usageStatus.resetInSeconds)} دیگر تلاش کنید.`
      );
      return;
    }

    const newMessages: ChatMessage[] = [
      ...chatMessages,
      { content: text, role: "user" },
    ];
    setChatMessages(newMessages);
    setChatInput("");
    setIsChatLoading(true);

    try {
      const reply = await chatMutation({
        messages: newMessages,
        systemContext: formattedSystemContext,
      });
      setChatMessages((prev) => [
        ...prev,
        { content: reply, role: "assistant" },
      ]);
    } catch (err: unknown) {
      const errorMsg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : String(err);
      toast.error(errorMsg || "Failed to connect to AI Assistant.");
      setChatMessages((prev) => [
        ...prev,
        {
          content: errorMsg.includes("سقف مجاز")
            ? errorMsg
            : "متاسفانه در برقراری ارتباط با سرویس هوش مصنوعی مشکلی پیش آمد. لطفاً اتصال اینترنت خود را بررسی کرده و دوباره امتحان کنید.",
          role: "assistant",
        },
      ]);
    } finally {
      setIsChatLoading(false);
      refetchUsageStatus();
    }
  };

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-hidden bg-background/50 p-3 sm:p-4">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-3">
          <div className="flex size-10 items-center justify-center rounded-xl border border-primary/25 bg-primary/15 text-primary shadow-primary/20 shadow-sm">
            <HugeiconsIcon icon={SparklesIcon} size={22} strokeWidth={2.5} />
          </div>
          <div>
            <h1 className="font-extrabold text-lg tracking-tight sm:text-xl">
              Glitchy AI Assistant
            </h1>
            <p className="text-muted-foreground text-xs">
              Smart Minecraft crash diagnostics, system-tailored FPS advice, and
              assistance
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {usageStatus && (
            <Badge
              className={cn(
                "flex items-center gap-1.5 px-2.5 py-1 font-medium text-xs transition-colors",
                usageStatus.remainingRequests === 0
                  ? "border-destructive/40 bg-destructive/10 text-destructive"
                  : usageStatus.remainingRequests <= 5
                    ? "border-amber-500/40 bg-amber-500/10 text-amber-400"
                    : "border-primary/30 bg-primary/10 text-primary"
              )}
              variant="outline"
            >
              <HugeiconsIcon icon={SparklesIcon} size={13} />
              <span>
                {usageStatus.remainingRequests}/{usageStatus.maxRequests}{" "}
                درخواست
              </span>
              {usageStatus.remainingRequests === 0 ? (
                <span className="text-[10px] opacity-90">
                  ({formatResetTime(usageStatus.resetInSeconds)})
                </span>
              ) : (
                <span className="hidden text-[10px] opacity-75 sm:inline">
                  (۵ ساعته)
                </span>
              )}
            </Badge>
          )}
        </div>
      </div>

      {/* Permission & Privacy Consent Banner */}
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-border/50 bg-secondary/25 px-4 py-2.5 backdrop-blur-md">
        <div className="flex items-center gap-2.5">
          <div className="flex size-8 items-center justify-center rounded-lg border border-primary/20 bg-primary/10 text-primary">
            <HugeiconsIcon icon={Shield01Icon} size={18} />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="font-bold text-foreground text-xs">
                Include System & Game Diagnostics
              </span>
              <Badge
                className="px-1.5 py-0 font-medium text-[10px]"
                variant="secondary"
              >
                Privacy Protected
              </Badge>
            </div>
            <p className="text-[11px] text-muted-foreground">
              Allows AI to inspect CPU, RAM, GPU, game logs, and mods to
              pinpoint issues accurately.
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <Dialog>
            <DialogTrigger
              render={
                <Button
                  className="h-8 gap-1.5 border-border/50 bg-background/50 text-xs hover:bg-secondary/60"
                  onClick={() => refetchSysData()}
                  size="sm"
                  variant="outline"
                >
                  <HugeiconsIcon icon={ViewIcon} size={14} />
                  <span>View Shared Data</span>
                </Button>
              }
            />
            <DialogContent className="flex max-h-[85vh] max-w-md flex-col border-border/60 bg-secondary/95 p-5">
              <DialogHeader className="pb-2">
                <DialogTitle className="flex items-center gap-2 font-bold text-base">
                  <HugeiconsIcon
                    className="text-primary"
                    icon={InformationCircleIcon}
                    size={18}
                  />
                  <span>Diagnostic Data Sent to AI</span>
                </DialogTitle>
                <DialogDescription className="text-muted-foreground text-xs">
                  Below is the exact system and Minecraft environment summary
                  shared with the AI assistant when consent is enabled. No
                  personal files or credentials are ever accessed.
                </DialogDescription>
              </DialogHeader>

              <div className="min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
                {isSysDataLoading ? (
                  <div className="flex items-center justify-center py-8">
                    <Spinner className="size-6" />
                  </div>
                ) : sysData ? (
                  <>
                    <div className="grid grid-cols-2 gap-2 text-xs">
                      <div className="rounded-lg border border-border/40 bg-background/40 p-2.5">
                        <span className="block text-[10px] text-muted-foreground">
                          OS
                        </span>
                        <span className="font-semibold text-foreground">
                          {sysData.os}
                        </span>
                      </div>
                      <div className="rounded-lg border border-border/40 bg-background/40 p-2.5">
                        <span className="block text-[10px] text-muted-foreground">
                          CPU
                        </span>
                        <span
                          className="block truncate font-semibold text-foreground"
                          title={sysData.cpu}
                        >
                          {sysData.cpu}
                        </span>
                      </div>
                      <div className="rounded-lg border border-border/40 bg-background/40 p-2.5">
                        <span className="block text-[10px] text-muted-foreground">
                          Physical RAM
                        </span>
                        <span className="font-semibold text-foreground">
                          {sysData.totalRamMb} MB (Free: {sysData.freeRamMb} MB)
                        </span>
                      </div>
                      <div className="rounded-lg border border-border/40 bg-background/40 p-2.5">
                        <span className="block text-[10px] text-muted-foreground">
                          Allocated Game RAM
                        </span>
                        <span className="font-semibold text-foreground">
                          {sysData.allocatedRamMb} MB
                        </span>
                      </div>
                      <div className="col-span-2 rounded-lg border border-border/40 bg-background/40 p-2.5">
                        <span className="block text-[10px] text-muted-foreground">
                          GPU / Graphics Adapter
                        </span>
                        <span className="font-semibold text-foreground">
                          {sysData.gpu}
                        </span>
                      </div>
                      <div className="col-span-2 rounded-lg border border-border/40 bg-background/40 p-2.5">
                        <span className="block text-[10px] text-muted-foreground">
                          Selected Java Runtime
                        </span>
                        <span className="block truncate font-mono text-[11px] text-foreground">
                          {sysData.selectedJava}
                        </span>
                      </div>
                    </div>

                    <div className="rounded-lg border border-border/40 bg-background/40 p-2.5">
                      <div className="mb-1.5 flex items-center justify-between">
                        <span className="text-[10px] text-muted-foreground">
                          Detected Installed Mods (
                          {sysData.installedMods.length})
                        </span>
                      </div>
                      {sysData.installedMods.length > 0 ? (
                        <div className="max-h-32 space-y-0.5 overflow-y-auto font-mono text-[10px] text-muted-foreground/90">
                          {sysData.installedMods.map((mod, i) => (
                            <div className="truncate" key={`mod-${mod}-${i}`}>
                              • {mod}
                            </div>
                          ))}
                        </div>
                      ) : (
                        <span className="text-[11px] text-muted-foreground italic">
                          No mods found in mods folder.
                        </span>
                      )}
                    </div>
                  </>
                ) : (
                  <p className="text-muted-foreground text-xs">
                    Unable to read system specs.
                  </p>
                )}
              </div>
            </DialogContent>
          </Dialog>

          <div className="flex items-center gap-2">
            <span className="font-medium text-muted-foreground text-xs">
              {shareDiagnostics ? "Enabled" : "Disabled"}
            </span>
            <Switch
              checked={shareDiagnostics}
              onCheckedChange={handleToggleDiagnostics}
            />
          </div>
        </div>
      </div>

      {/* Full-width Tabs */}
      <Tabs
        className="flex h-full min-h-0 flex-1 flex-col"
        onValueChange={setActiveTab}
        value={activeTab}
      >
        <TabsList className="grid w-full grid-cols-2 border border-border/40 bg-secondary/50 p-1">
          <TabsTrigger
            className="flex items-center justify-center gap-2 py-2 font-semibold text-xs"
            value="diagnostic"
          >
            <HugeiconsIcon icon={Wrench01Icon} size={16} />
            <span>Diagnostics & Auto-Fix</span>
          </TabsTrigger>
          <TabsTrigger
            className="flex items-center justify-center gap-2 py-2 font-semibold text-xs"
            value="chat"
          >
            <HugeiconsIcon icon={Message01Icon} size={16} />
            <span>AI Chat & Guide</span>
          </TabsTrigger>
        </TabsList>

        {/* TAB 1: Diagnostics (Full-width) */}
        <TabsContent
          className="mt-3 min-h-0 flex-1 overflow-y-auto"
          value="diagnostic"
        >
          <div className="flex w-full flex-col gap-4 pb-6">
            {/* Action Trigger Card */}
            <Card className="w-full border-border/60 bg-secondary/20 backdrop-blur-md">
              <CardHeader className="pb-3">
                <CardTitle className="flex items-center gap-2 font-bold text-base">
                  <HugeiconsIcon
                    className="text-primary"
                    icon={CpuIcon}
                    size={18}
                  />
                  <span>Scan & Diagnose Last Game Crash</span>
                </CardTitle>
                <CardDescription className="text-xs leading-relaxed">
                  If your Minecraft crashed, exited with Code 1, or failed to
                  launch due to Fabric/Forge mod conflicts, Glitchy AI will
                  inspect the latest crash reports, loader logs, and mods folder
                  to identify the exact cause and offer a 1-click automatic fix.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap items-center gap-3 pt-0">
                <Button
                  className="bg-primary px-5 py-2.5 font-bold text-primary-foreground text-xs shadow-md shadow-primary/25 hover:brightness-110"
                  disabled={
                    isDiagnosing || usageStatus?.remainingRequests === 0
                  }
                  onClick={() => handleDiagnose(false)}
                >
                  {isDiagnosing ? (
                    <>
                      <Spinner className="mr-2 size-4" />
                      <span>Analyzing Crash with AI...</span>
                    </>
                  ) : (
                    <>
                      <HugeiconsIcon
                        className="mr-2"
                        icon={SparklesIcon}
                        size={16}
                      />
                      <span>Analyze Last Crash Automatically</span>
                    </>
                  )}
                </Button>

                {usageStatus?.remainingRequests === 0 ? (
                  <span className="font-medium text-destructive text-xs">
                    (سقف ۲۰ درخواست در ۵ ساعت تکمیل شده است. بازنشانی:{" "}
                    {formatResetTime(usageStatus.resetInSeconds)})
                  </span>
                ) : (
                  <span className="text-muted-foreground text-xs">
                    Or paste a specific error / log below:
                  </span>
                )}
              </CardContent>
            </Card>

            {/* Manual Log Input Accordion */}
            <div className="w-full rounded-xl border border-border/40 bg-secondary/15 p-3.5">
              <label
                className="mb-1.5 block font-semibold text-muted-foreground text-xs"
                htmlFor="manual-log-area"
              >
                Manual Log or Error Input (Optional):
              </label>
              <Textarea
                className="h-20 resize-none border-border/40 bg-background/50 font-mono text-[11px]"
                id="manual-log-area"
                onChange={(e) => setManualLog(e.target.value)}
                placeholder="Paste your Minecraft error or log snippet here..."
                value={manualLog}
              />
              {manualLog.trim() && (
                <Button
                  className="mt-2 text-xs"
                  disabled={isDiagnosing}
                  onClick={() => handleDiagnose(true)}
                  size="sm"
                  variant="secondary"
                >
                  {isDiagnosing ? (
                    <Spinner className="mr-1.5 size-3.5" />
                  ) : null}
                  <span>Diagnose This Text</span>
                </Button>
              )}
            </div>

            {/* Diagnosis Result Display */}
            {diagnosis && (
              <Card className="fade-in-50 w-full animate-in overflow-hidden border-border/60 bg-secondary/25 backdrop-blur-md duration-200">
                <CardHeader className="border-border/40 border-b bg-secondary/40 pb-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <CardTitle className="flex items-center gap-2 font-bold text-sm">
                      <HugeiconsIcon
                        className="text-emerald-400"
                        icon={CheckmarkCircle02Icon}
                        size={18}
                      />
                      <span>AI Diagnosis Report</span>
                    </CardTitle>

                    {/* Detected issue tag */}
                    <div className="flex items-center gap-2">
                      <span className="text-[11px] text-muted-foreground">
                        Issue Type:
                      </span>
                      <Badge
                        className="border-primary/30 bg-primary/10 font-bold font-mono text-primary text-xs"
                        variant="outline"
                      >
                        {diagnosis.detectedIssueType}
                      </Badge>
                      <Badge
                        className={cn(
                          "text-[10px]",
                          diagnosis.source === "MOD_LOADER_CRASH" &&
                            "border-amber-500/30 bg-amber-500/15 text-amber-300"
                        )}
                        variant="secondary"
                      >
                        Source: {diagnosis.source}
                      </Badge>
                    </div>
                  </div>
                </CardHeader>

                <CardContent className="space-y-4 pt-4">
                  {/* Mod Loader Notice */}
                  {diagnosis.source === "MOD_LOADER_CRASH" && (
                    <div className="rounded-xl border border-amber-500/30 bg-amber-500/10 p-3 text-amber-200/90 text-xs leading-relaxed">
                      <strong>Mod Loader Conflict:</strong> Minecraft did not
                      generate a standard crash report because the crash
                      occurred in the Fabric / Forge loader during early startup
                      (e.g. incompatible mod version or missing dependency).
                      Glitchy AI analyzed the loader log and mods folder.
                    </div>
                  )}

                  {/* AI Explanation Box */}
                  <div className="rounded-xl border border-primary/20 bg-primary/5 p-4 font-sans text-foreground text-xs leading-relaxed sm:text-sm">
                    <BidiMessageView content={diagnosis.aiExplanation} />
                  </div>

                  {/* 1-Click Auto-Fix Action */}
                  {diagnosis.autoFixAvailable && (
                    <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-4">
                      <div className="flex items-center gap-3">
                        <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-emerald-500/20 text-emerald-400">
                          <HugeiconsIcon icon={RepairIcon} size={18} />
                        </div>
                        <div>
                          <h4 className="font-bold text-emerald-300 text-xs sm:text-sm">
                            Automatic Fix Available!
                          </h4>
                          <p className="text-[11px] text-emerald-200/70">
                            {diagnosis.autoFixDescription ??
                              "Click the button to automatically apply recommended settings."}
                          </p>
                        </div>
                      </div>

                      <Button
                        className="bg-emerald-600 px-5 font-bold text-white text-xs shadow-emerald-900/30 shadow-lg hover:bg-emerald-500"
                        disabled={isFixing}
                        onClick={handleAutoFix}
                      >
                        {isFixing ? (
                          <>
                            <Spinner className="mr-1.5 size-4" />
                            <span>Applying Fix...</span>
                          </>
                        ) : (
                          <>
                            <HugeiconsIcon
                              className="mr-1.5"
                              icon={RepairIcon}
                              size={15}
                            />
                            <span>Apply 1-Click Fix</span>
                          </>
                        )}
                      </Button>
                    </div>
                  )}

                  {/* Collapsible log snippet */}
                  <details className="text-xs">
                    <summary className="cursor-pointer py-1 font-medium text-muted-foreground hover:text-foreground">
                      View Analyzed Log Snippet ({diagnosis.source})
                    </summary>
                    <pre className="mt-2 max-h-48 overflow-auto rounded-lg border border-border/40 bg-black/50 p-3 font-mono text-[10px] text-zinc-300">
                      {diagnosis.logSnippet}
                    </pre>
                  </details>
                </CardContent>
              </Card>
            )}
          </div>
        </TabsContent>

        {/* TAB 2: Interactive AI Chat (Full-width, smoothly scrollable) */}
        <TabsContent
          className="mt-3 flex min-h-0 flex-1 flex-col overflow-hidden"
          value="chat"
        >
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-border/50 bg-secondary/20 backdrop-blur-md">
            {/* Chat toolbar / Reset */}
            <div className="flex items-center justify-between border-border/40 border-b bg-secondary/30 px-4 py-2 text-xs">
              <div className="flex items-center gap-2 text-muted-foreground">
                <HugeiconsIcon icon={Message01Icon} size={14} />
                <span className="font-medium text-[11px]">
                  Chat History Saved
                </span>
              </div>
              <Button
                className="h-7 gap-1.5 px-2.5 text-muted-foreground text-xs hover:bg-secondary/60 hover:text-foreground"
                disabled={isChatLoading || chatMessages.length <= 1}
                onClick={handleResetChat}
                size="sm"
                variant="ghost"
              >
                <HugeiconsIcon icon={RefreshIcon} size={13} />
                <span>شروع چت جدید</span>
              </Button>
            </div>

            {/* Scrollable Message History */}
            <div className="min-h-0 flex-1 space-y-3.5 overflow-y-auto p-4">
              {chatMessages.map((msg, idx) => (
                <div
                  className={cn(
                    "flex items-start gap-2.5",
                    msg.role === "user" ? "flex-row-reverse" : "flex-row"
                  )}
                  key={`msg-${idx}-${msg.role}`}
                >
                  <div
                    className={cn(
                      "flex size-7 shrink-0 items-center justify-center rounded-lg font-bold text-xs",
                      msg.role === "user"
                        ? "bg-primary text-primary-foreground"
                        : "border border-border/60 bg-secondary/80 text-foreground"
                    )}
                  >
                    {msg.role === "user" ? "You" : "AI"}
                  </div>

                  <div
                    className={cn(
                      "max-w-[85%] rounded-2xl px-4 py-2.5 text-xs leading-relaxed sm:max-w-[75%] sm:text-sm",
                      msg.role === "user"
                        ? "rounded-tr-none bg-primary text-primary-foreground shadow-sm"
                        : "rounded-tl-none border border-border/40 bg-secondary/40 text-foreground shadow-sm"
                    )}
                  >
                    <BidiMessageView content={msg.content} />
                  </div>
                </div>
              ))}

              {isChatLoading && (
                <div className="flex items-center gap-2 p-2 text-muted-foreground text-xs">
                  <Spinner className="mr-1 size-3.5" />
                  <span>Glitchy AI is thinking...</span>
                </div>
              )}
              <div ref={messagesEndRef} />
            </div>

            {/* Rate limit warning banner if exhausted */}
            {usageStatus?.remainingRequests === 0 && (
              <div className="mx-3 my-2 flex items-center justify-between gap-2 rounded-lg border border-destructive/30 bg-destructive/10 px-3.5 py-2 text-destructive text-xs">
                <div className="flex items-center gap-2">
                  <HugeiconsIcon
                    className="size-4 shrink-0"
                    icon={Shield01Icon}
                  />
                  <span>
                    سقف مجاز ۲۰ درخواست هوش مصنوعی در ۵ ساعت تکمیل شده است. زمان
                    بازنشانی: {formatResetTime(usageStatus.resetInSeconds)}.
                  </span>
                </div>
              </div>
            )}

            {/* Suggested quick prompt pills */}
            <div className="scrollbar-none flex gap-2 overflow-x-auto border-border/30 border-t bg-background/20 px-4 py-2">
              {(isFaLocale() ? SUGGESTED_PROMPTS_FA : SUGGESTED_PROMPTS_EN).map(
                (prompt) => (
                  <button
                    className="shrink-0 rounded-full border border-border/50 bg-secondary/30 px-3 py-1 text-[11px] text-muted-foreground transition-all hover:bg-secondary/60 hover:text-foreground disabled:opacity-40"
                    dir="auto"
                    disabled={
                      isChatLoading || usageStatus?.remainingRequests === 0
                    }
                    key={prompt}
                    onClick={() => handleSendMessage(prompt)}
                    type="button"
                  >
                    {prompt}
                  </button>
                )
              )}
            </div>

            {/* Input Bar */}
            <div className="flex items-center gap-2 border-border/40 border-t bg-secondary/30 p-3">
              <Textarea
                className="max-h-24 min-h-[42px] resize-none border-border/40 bg-background/60 py-2.5 text-xs"
                disabled={usageStatus?.remainingRequests === 0}
                onChange={(e) => setChatInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    handleSendMessage();
                  }
                }}
                placeholder={
                  usageStatus?.remainingRequests === 0
                    ? "سقف مجاز ۲۰ درخواست در ۵ ساعت تکمیل شده است..."
                    : "Ask anything about Minecraft, mods, or settings... (Press Enter to send)"
                }
                rows={1}
                value={chatInput}
              />
              <Button
                className="size-10 shrink-0 rounded-xl bg-primary p-0 text-primary-foreground hover:brightness-110"
                disabled={
                  !chatInput.trim() ||
                  isChatLoading ||
                  usageStatus?.remainingRequests === 0
                }
                onClick={() => handleSendMessage()}
              >
                {isChatLoading ? (
                  <Spinner className="size-4" />
                ) : (
                  <HugeiconsIcon icon={SentIcon} size={18} />
                )}
              </Button>
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}
