"use client";

import {
  type ComponentProps,
  type ReactNode,
  useState,
  useTransition,
} from "react";
import { toast } from "sonner";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { LoadingSwap } from "@/components/ui/animated/swapper";
import { Button } from "@/components/ui/button";

export function ActionButton({
  action,
  disabled,
  requireAreYouSure = false,
  areYouSureDescription = "This action is irreversible.",
  areYouSureButton = "Confirm",
  ...props
}: ComponentProps<typeof Button> & {
  action: () =>
    | Promise<{ error: boolean; message?: string }>
    | Promise<void>
    | void;
  requireAreYouSure?: boolean;
  areYouSureDescription?: ReactNode;
  areYouSureButton?: ReactNode;
}) {
  const [isLoading, startTransition] = useTransition();
  const [open, setOpen] = useState(false);

  function performAction() {
    return new Promise<void>((resolve) => {
      startTransition(async () => {
        try {
          const data = await action();

          if (data?.error) {
            toast.error(data.message ?? "Error");
          }
        } catch (error) {
          console.error(error);
        } finally {
          resolve();
        }
      });
    });
  }

  if (requireAreYouSure) {
    return (
      <AlertDialog onOpenChange={setOpen} open={open}>
        <AlertDialogTrigger render={<Button {...props} />} />
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Are you sure?</AlertDialogTitle>
            <AlertDialogDescription>
              {areYouSureDescription}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Back</AlertDialogCancel>
            <AlertDialogAction
              disabled={isLoading || disabled}
              onClick={async () => {
                await performAction();
                setOpen(false);
              }}
              type="button"
            >
              <LoadingSwap isLoading={isLoading}>
                {areYouSureButton}
              </LoadingSwap>
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    );
  }

  return (
    <Button
      {...props}
      disabled={disabled || isLoading}
      onClick={(e) => {
        performAction();
        props.onClick?.(e);
      }}
    >
      <LoadingSwap
        className="inline-flex items-center gap-2"
        isLoading={isLoading}
      >
        {props.children}
      </LoadingSwap>
    </Button>
  );
}
