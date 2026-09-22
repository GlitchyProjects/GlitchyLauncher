import {
  Globe02Icon,
  LockIcon,
  UserGroupIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Plus, Shield, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/stores/locale";

export interface CommunityGroup {
  createdAt: number;
  description: string;
  id: string;
  inviteCode: string;
  membersCount: number;
  name: string;
  ownerId: string;
  visibility: "public" | "private";
}

interface CreateGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreateGroup: (
    group: Omit<
      CommunityGroup,
      "id" | "inviteCode" | "createdAt" | "membersCount"
    >
  ) => void;
  userCreatedCount: number;
}

export function CreateGroupModal({
  isOpen,
  onClose,
  onCreateGroup,
  userCreatedCount,
}: CreateGroupModalProps) {
  const { locale } = useLocale();
  const isFa = locale === "fa";

  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [visibility, setVisibility] = useState<"public" | "private">("public");

  if (!isOpen) {
    return null;
  }

  const isMaxReached = userCreatedCount >= 2;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isMaxReached) {
      toast.error(
        isFa
          ? "شما به سقف مجاز ۲ گروه ایجاد شده رسیده‌اید."
          : "You have reached the maximum limit of 2 created communities."
      );
      return;
    }
    if (!name.trim()) {
      toast.error(
        isFa ? "نام گروه الزامی است." : "Community name is required."
      );
      return;
    }

    onCreateGroup({
      description: description.trim(),
      name: name.trim(),
      ownerId: "current_user",
      visibility,
    });

    setName("");
    setDescription("");
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex animate-fade-in items-center justify-center bg-black/60 p-4 backdrop-blur-xs">
      <div className="relative w-full max-w-md animate-scale-up overflow-hidden rounded-3xl border border-border/60 bg-secondary/35 p-6 shadow-2xl backdrop-blur-xl">
        <button
          className="absolute top-4 right-4 rounded-full border border-border/40 bg-background/50 p-1.5 text-muted-foreground transition-colors hover:bg-background hover:text-foreground"
          onClick={onClose}
          type="button"
        >
          <X className="size-4" />
        </button>

        <div className="flex items-center gap-3">
          <div className="flex size-10 items-center justify-center rounded-2xl border border-primary/40 bg-primary/10 text-primary">
            <HugeiconsIcon icon={UserGroupIcon} size={20} strokeWidth={2} />
          </div>
          <div>
            <h2 className="font-bold text-foreground text-lg">
              {isFa ? "ایجاد گروه کامیونیتی" : "Create Community"}
            </h2>
            <p className="text-muted-foreground text-xs">
              {isFa
                ? `حداکثر ۲ گروه (ایجاد شده: ${userCreatedCount}/۲)`
                : `Max 2 communities allowed (Created: ${userCreatedCount}/2)`}
            </p>
          </div>
        </div>

        {isMaxReached ? (
          <div className="my-6 space-y-2 rounded-2xl border border-destructive/30 bg-destructive/10 p-4 text-center">
            <Shield className="mx-auto size-6 text-destructive" />
            <p className="font-semibold text-destructive text-xs">
              {isFa
                ? "شما قبلاً ۲ گروه ایجاد کرده‌اید و به سقف مجاز رسیده‌اید."
                : "You have already created 2 communities (Limit reached)."}
            </p>
            <p className="text-[11px] text-muted-foreground">
              {isFa
                ? "برای ساخت گروه جدید، یکی از گروه‌های قبلی خود را حذف کنید."
                : "To create a new community, delete one of your existing ones."}
            </p>
          </div>
        ) : (
          <form className="mt-5 space-y-4" onSubmit={handleSubmit}>
            <div className="space-y-1.5">
              <Label className="font-semibold text-xs" htmlFor="group-name">
                {isFa ? "نام گروه" : "Community Name"}
              </Label>
              <Input
                id="group-name"
                maxLength={32}
                onChange={(e) => setName(e.target.value)}
                placeholder={
                  isFa
                    ? "مثلاً Glitchy SMP یا ردستون‌کاران"
                    : "e.g. Glitchy SMP or Redstone Builders"
                }
                required
                value={name}
              />
            </div>

            <div className="space-y-1.5">
              <Label className="font-semibold text-xs" htmlFor="group-desc">
                {isFa ? "توضیحات کوتاه" : "Description"}
              </Label>
              <Textarea
                className="resize-none text-xs"
                id="group-desc"
                maxLength={120}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={
                  isFa
                    ? "هدف گروه، قوانین یا مشخصات سرور..."
                    : "Server info, purpose of this community..."
                }
                rows={3}
                value={description}
              />
            </div>

            {/* Visibility Option */}
            <div className="space-y-2">
              <Label className="font-semibold text-xs">
                {isFa ? "سطح دسترسی و نمایش" : "Visibility"}
              </Label>
              <div className="grid grid-cols-2 gap-2">
                <button
                  className={`flex flex-col items-center justify-center gap-1.5 rounded-2xl border p-3 text-center transition-all ${
                    visibility === "public"
                      ? "border-primary bg-primary/15 text-primary shadow-xs"
                      : "border-border/40 bg-background/30 text-muted-foreground hover:bg-background/60"
                  }`}
                  onClick={() => setVisibility("public")}
                  type="button"
                >
                  <HugeiconsIcon icon={Globe02Icon} size={18} strokeWidth={2} />
                  <span className="font-bold text-xs">
                    {isFa ? "عمومی (Public)" : "Public"}
                  </span>
                  <span className="text-[10px] opacity-75">
                    {isFa ? "قابل جستجو برای همه" : "Listed in Discover"}
                  </span>
                </button>

                <button
                  className={`flex flex-col items-center justify-center gap-1.5 rounded-2xl border p-3 text-center transition-all ${
                    visibility === "private"
                      ? "border-primary bg-primary/15 text-primary shadow-xs"
                      : "border-border/40 bg-background/30 text-muted-foreground hover:bg-background/60"
                  }`}
                  onClick={() => setVisibility("private")}
                  type="button"
                >
                  <HugeiconsIcon icon={LockIcon} size={18} strokeWidth={2} />
                  <span className="font-bold text-xs">
                    {isFa ? "خصوصی (Private)" : "Private"}
                  </span>
                  <span className="text-[10px] opacity-75">
                    {isFa ? "فقط با لینک دعوت" : "Invite Link Only"}
                  </span>
                </button>
              </div>
            </div>

            <Button className="w-full gap-2 font-bold" type="submit">
              <Plus className="size-4" />
              <span>{isFa ? "ساخت گروه" : "Create Community"}</span>
            </Button>
          </form>
        )}
      </div>
    </div>
  );
}
