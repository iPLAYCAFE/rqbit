import { useState, useContext, useEffect } from "react";
import { RqbitWebUI } from "rqbit-webui/src/rqbit-web";
import { CurrentDesktopState, RqbitDesktopConfig } from "./configuration";
import { ConfigModal } from "./configure";
import { IconButton } from "rqbit-webui/src/components/buttons/IconButton";
import { BsSliders2, BsListTask } from "react-icons/bs";
import { APIContext } from "rqbit-webui/src/context";
import { CreateTorrentQueueModal } from "rqbit-webui/src/components/modal/CreateTorrentQueueModal";
import { makeAPI } from "./api";

const QueueButton = ({ onClick }: { onClick: () => void }) => {
  const API = useContext(APIContext);
  const [count, setCount] = useState<number>(0);

  useEffect(() => {
    const fetch = async () => {
      try {
        const tasks = await API.listCreateTorrentTasks();
        const pendingOrProcessing = tasks.filter(t => t.status === "pending" || t.status === "processing").length;
        setCount(pendingOrProcessing);
      } catch (e) {
        console.error(e);
      }
    };
    fetch();
    const interval = setInterval(fetch, 2000);
    return () => clearInterval(interval);
  }, [API]);

  return (
    <div className="relative">
      <IconButton onClick={onClick} title="Torrent Creation Queue">
        <BsListTask />
      </IconButton>
      {count > 0 && (
        <span className="absolute -top-1 -right-1 bg-red-500 text-white text-[10px] font-bold px-1.5 rounded-full min-w-[16px] h-[16px] flex items-center justify-center">
          {count}
        </span>
      )}
    </div>
  );
};

export const RqbitDesktop: React.FC<{
  version: string;
  defaultConfig: RqbitDesktopConfig;
  currentState: CurrentDesktopState;
}> = ({ version, defaultConfig, currentState }) => {
  let [configured, setConfigured] = useState<boolean>(currentState.configured);
  let [config, setConfig] = useState<RqbitDesktopConfig>(
    currentState.config ?? defaultConfig,
  );
  const [configurationOpened, setConfigurationOpened] = useState<boolean>(false);
  const [queueOpened, setQueueOpened] = useState<boolean>(false);

  const configButton = (
    <div className="flex gap-2">
      <QueueButton onClick={() => setQueueOpened(true)} />
      <IconButton
        onClick={() => {
          setConfigurationOpened(true);
        }}
        title="Configure"
      >
        <BsSliders2 />
      </IconButton>
    </div>
  );

  return (
    <APIContext.Provider value={makeAPI(config)}>
      {configured && (
        <RqbitWebUI
          title={`Rqbit Desktop`}
          version={version}
          menuButtons={[configButton]}
        ></RqbitWebUI>
      )}
      <ConfigModal
        show={!configured || configurationOpened}
        handleStartReconfigure={() => {
          setConfigured(false);
        }}
        handleCancel={() => {
          setConfigurationOpened(false);
        }}
        handleConfigured={(config) => {
          setConfig(config);
          setConfigurationOpened(false);
          setConfigured(true);
        }}
        initialConfig={config}
        defaultConfig={defaultConfig}
      />
      {queueOpened && (
        <CreateTorrentQueueModal onHide={() => setQueueOpened(false)} />
      )}
    </APIContext.Provider>
  );
};
