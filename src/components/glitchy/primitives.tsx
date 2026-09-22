import type React from "react";
import { cn } from "@/lib/utils";

export function GlassPanel({
  className,
  variant = "default",
  ...props
}: React.HTMLAttributes<HTMLDivElement> & { variant?: "default" | "strong" }) {
  return (
    <div
      className={cn(
        variant === "strong" ? "glass-strong" : "glass",
        "rounded-2xl",
        className
      )}
      {...props}
    />
  );
}

export function SectionHeading({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <h3
      className={cn(
        "font-bold text-gradient-blue text-xs uppercase tracking-[0.18em]",
        className
      )}
    >
      {children}
    </h3>
  );
}

export function GlowButton({
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={cn(
        "relative inline-flex items-center justify-center gap-2 rounded-xl",
        "bg-gradient-to-br from-[#2484EC] to-[#247CD4]",
        "px-6 py-3 font-bold text-white",
        "shadow-[0_0_0_1px_rgba(36,132,236,0.4),0_0_24px_rgba(36,132,236,0.35)]",
        "transition-all duration-200 hover:shadow-[0_0_0_1px_rgba(36,132,236,0.6),0_0_32px_rgba(36,132,236,0.5)]",
        "hover:-translate-y-0.5 active:translate-y-0",
        "disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:translate-y-0",
        className
      )}
      {...props}
    />
  );
}

export function StatTile({
  value,
  label,
  className,
}: {
  value: React.ReactNode;
  label: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "glass flex flex-col items-center justify-center gap-1 rounded-xl p-4 text-center",
        className
      )}
    >
      <div className="font-black text-3xl text-gradient-blue tabular-nums leading-none">
        {value}
      </div>
      <div className="font-medium text-[10px] text-muted-foreground uppercase tracking-[0.16em]">
        {label}
      </div>
    </div>
  );
}
