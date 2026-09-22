import { useState } from "react";
import { DownloadsPanel } from "@/components/blocks/downloads/downloads-panel";
import { LegendsDownloads } from "@/components/blocks/downloads/legends";
import { PvpClientDownloads } from "@/components/blocks/downloads/pvp-clients";
import { StepConfigureInstance } from "@/components/blocks/downloads/step-configure-instance";
import { StepInstalling } from "@/components/blocks/downloads/step-installing";
import { StepSelectLoader } from "@/components/blocks/downloads/step-select-loader";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useBackendMutation } from "@/hooks/use-backend";
import type { VersionLoader } from "@/invokes";
import { useConfig } from "@/stores/config";
import { useDownloads } from "@/stores/downloads";

export type LoaderType = "vanilla" | "fabric" | "forge" | "optifine";
export type WizardStep = 1 | 2 | 3;

export default function Downloads() {
  return (
    <div className="flex h-full min-h-0 w-full flex-col gap-3">
      <DownloadsPanel />
      <Tabs className="flex min-h-0 flex-1 flex-col" defaultValue="minecraft">
        <div className="w-full shrink-0 overflow-x-auto pb-1">
          <TabsList className="w-full bg-secondary/40 backdrop-blur-md">
            <TabsTrigger
              className="flex-1 min-w-max whitespace-nowrap"
              value="minecraft"
            >
              Minecraft
            </TabsTrigger>
            <TabsTrigger
              className="flex-1 min-w-max whitespace-nowrap"
              value="legends"
            >
              Legends
            </TabsTrigger>
            <TabsTrigger
              className="flex-1 min-w-max whitespace-nowrap"
              value="pvp"
            >
              PvP Clients
            </TabsTrigger>
          </TabsList>
        </div>
        <TabsContent className="min-h-0 flex-1" value="minecraft">
          <InstallerWizard />
        </TabsContent>
        <TabsContent className="min-h-0 flex-1" value="legends">
          <LegendsDownloads />
        </TabsContent>
        <TabsContent className="min-h-0 flex-1" value="pvp">
          <PvpClientDownloads />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function InstallerWizard() {
  const [step, setStep] = useState<WizardStep>(1);
  const [activeLoader, setActiveLoader] = useState<LoaderType>("vanilla");
  const [instanceName, setInstanceName] = useState<string>("");
  const [sessionId, setSessionId] = useState<string | null>(null);

  const upsert = useDownloads((s) => s.upsert);
  const { mutateAsync: downloadVersion } = useBackendMutation({
    name: "download_version",
  });

  const handleSelectLoader = (loader: LoaderType) => {
    setActiveLoader(loader);
    setStep(2);
  };

  const { setVersion } = useConfig();

  const handleStartInstall = async (version: VersionLoader, name: string) => {
    setInstanceName(name);
    setStep(3);
    setVersion(version.id);

    try {
      // Returns immediately with the session id — the install itself
      // runs on a background task in the backend and keeps going even
      // if the user leaves this page (or closes the wizard).
      const id = await downloadVersion({
        name,
        versionLoader: version,
      });
      setSessionId(id);
      // Seed the store so step 3 renders instantly; live updates arrive
      // via the global download-session listener.
      upsert({
        bytesPerSecond: 0,
        createdAt: Math.floor(Date.now() / 1000),
        currentFile: "",
        downloadedBytes: 0,
        error: null,
        id,
        label: name || version.id,
        percent: 0,
        phase: "prepare",
        state: "running",
        totalBytes: 0,
        versionId: version.id,
      });
    } catch {
      setStep(2);
    }
  };

  const handleReset = () => {
    setStep(1);
    setInstanceName("");
    setSessionId(null);
  };

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-2xl flex-col">
      <div className="flex min-h-[450px] w-full flex-col rounded-2xl border border-border/50 bg-secondary/20 p-6 shadow-xl backdrop-blur-md">
        {step === 1 && <StepSelectLoader onSelect={handleSelectLoader} />}
        {step === 2 && (
          <StepConfigureInstance
            activeLoader={activeLoader}
            onBack={() => setStep(1)}
            onStartInstall={handleStartInstall}
          />
        )}
        {step === 3 && (
          <StepInstalling
            instanceName={instanceName}
            onReset={handleReset}
            sessionId={sessionId}
          />
        )}
      </div>
    </div>
  );
}
