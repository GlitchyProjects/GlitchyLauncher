import errorsJson from "@/messages/errors.json";

const errors = new Map(Object.entries(errorsJson));

export interface ErrorDescriptor {
  description: string;
  title: string;
}

export const errorText = (code: string): ErrorDescriptor =>
  errors.get(code) ?? errorsJson.ERROR_UNKNOWN;

/**
 * Recovery action tokens emitted by the Rust backend alongside an
 * error. The frontend maps these to button labels and behaviors.
 */
export const RECOVERY_ACTIONS: Record<
  string,
  { label: string; action: string }
> = {
  ACTION_CREATE_PROFILE: { action: "create_profile", label: "Create Profile" },
  ACTION_INSTALL_JAVA: { action: "install_java", label: "Install Java" },
  ACTION_OPEN_DOWNLOADS: { action: "open_downloads", label: "Open Downloads" },
  ACTION_OPEN_MIRROR_SETTINGS: {
    action: "open_mirror_settings",
    label: "Mirror Settings",
  },
  ACTION_REPAIR_INSTALLATION: {
    action: "repair_installation",
    label: "Repair Installation",
  },
};

export const recoveryAction = (token: string | null | undefined) =>
  token && RECOVERY_ACTIONS[token] ? RECOVERY_ACTIONS[token] : null;
