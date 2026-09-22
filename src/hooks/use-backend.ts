import {
  type QueryKey,
  type UseMutationOptions,
  type UseQueryOptions,
  useMutation,
  useQuery,
} from "@tanstack/react-query";
import { sendNotification } from "@tauri-apps/plugin-notification";
import { toast } from "sonner";
import type { InvokeError, Invokes } from "@/invokes";
import { backend } from "@/lib/utils";
import { errorText, recoveryAction } from "@/messages";

export function useBackend<
  Invoke extends keyof Invokes,
  TData = Invokes[Invoke]["returns"],
>({
  name,
  args,
  ...params
}: Omit<
  UseQueryOptions<Invokes[Invoke]["returns"], InvokeError, TData>,
  "queryFn" | "queryKey"
> & { name: Invoke; args?: Invokes[Invoke]["args"]; queryKey?: QueryKey }) {
  const query = useQuery({
    queryFn: () => backend(name, args),
    queryKey: name.split("_"),
    ...params,
  });

  return query;
}

type TVarsType<
  Args extends Invokes[keyof Invokes]["args"],
  TArgs extends Partial<Record<string, unknown>>,
> = keyof Omit<Args, keyof TArgs> extends never
  ? void // No parameters required
  : Omit<Args, keyof TArgs>;

export function useBackendMutation<
  Invoke extends keyof Invokes,
  // biome-ignore lint/complexity/noBannedTypes: with {} every magical type works fine
  TArgs extends Partial<Invokes[Invoke]["args"]> = {},
>({
  name,
  args,
  onError,
  ...params
}: Omit<
  UseMutationOptions<
    Invokes[Invoke]["returns"], // TData
    InvokeError, // TError
    TVarsType<Invokes[Invoke]["args"], TArgs> // TVariables
  >,
  "mutationFn"
> & { name: Invoke; args?: TArgs }) {
  type TVars = TVarsType<Invokes[Invoke]["args"], TArgs>;

  const mutation = useMutation<
    Invokes[Invoke]["returns"], // TData
    InvokeError, // TError
    TVars // TVariables
  >({
    mutationFn: (variables: TVars) =>
      backend(name, { ...args, ...variables } as Invokes[Invoke]["args"]),
    mutationKey: name.split("_"),
    onError: (error, vars, res, context) => {
      if (onError) {
        onError(error, vars, res, context);
      } else {
        // Use the structured error envelope. `message` comes directly
        // from the Rust backend; `details` is optional technical info.
        // Fall back to the localized `errorText` lookup if the backend
        // didn't provide a message.
        const displayError = errorText(error.code);
        const title = error?.message || displayError.title;
        const description = error?.details
          ? `${displayError.description}\n${error.details}`
          : displayError.description;
        const recovery = recoveryAction(error.recovery);
        toast.error(title, {
          action: recovery
            ? {
                label: recovery.label,
                onClick: () => {
                  window.dispatchEvent(
                    new CustomEvent("recovery-action", {
                      detail: recovery.action,
                    })
                  );
                },
              }
            : undefined,
          description,
        });
        sendNotification({
          body: displayError.description,
          title: displayError.title,
        });
      }
    },
    ...params,
  });

  return mutation;
}
