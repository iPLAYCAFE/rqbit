
import { useContext, useEffect, useRef, useState } from "react";
import { generateMagnetLink } from "../../helper/magnetUtils";
import { APIContext } from "../../context";
import { ErrorComponent } from "../ErrorComponent";
import { ErrorWithLabel } from "../../rqbit-web";
import { Spinner } from "../Spinner";
import { Modal } from "./Modal";
import { ModalBody } from "./ModalBody";
import { ModalFooter } from "./ModalFooter";
import { Button } from "../buttons/Button";
import { FormInput } from "../forms/FormInput";
import { Form } from "../forms/Form";
import { useTorrentStore } from "../../stores/torrentStore";
import { FormCheckbox } from "../forms/FormCheckbox";
import { formatBytes } from "../../helper/formatBytes";

const STORAGE_KEY_TRACKERS = "rqbit_create_torrent_trackers";

const ProgressBar = ({ current, total }: { current: number; total: number }) => {
  const percent = total > 0 ? Math.round((current / total) * 100) : 0;
  return (
    <div className="w-full bg-gray-200 rounded-full h-2.5 dark:bg-gray-700 mb-2">
      <div
        className="bg-blue-600 h-2.5 rounded-full transition-all duration-300"
        style={{ width: `${percent}%` }}
      ></div>
      <div className="text-xs text-center mt-1 text-gray-500 dark:text-gray-400">
        {percent}% ({formatBytes(current)} / {formatBytes(total)})
      </div>
    </div>
  );
};

export const CreateTorrentModal = (props: { onHide: () => void }) => {
  const { onHide } = props;
  const [path, setPath] = useState("");
  const [trackers, setTrackers] = useState("");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<ErrorWithLabel | null>(null);
  const API = useContext(APIContext);
  const refreshTorrents = useTorrentStore((state) => state.refreshTorrents);
  const [skipHashCheck, setSkipHashCheck] = useState(true);
  const [progress, setProgress] = useState<{
    chunk: number;
    total: number;
    current: number;
  } | null>(null);

  const isTauri = useRef(!!(window as any).__TAURI_INTERNALS__);

  useEffect(() => {
    const stored = localStorage.getItem(STORAGE_KEY_TRACKERS);
    if (stored) {
      setTrackers(stored);
    }
  }, []);

  const handlePick = async (directory: boolean) => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        directory: directory,
      });
      if (selected) {
        setPath(selected as string);
      }
    } catch (e) {
      console.error("Error picking file/directory", e);
    }
  };

  if (progress && progress.chunk === progress.total && progress.total > 0 && !creating) {
      // Success state
      // Construct magnet link
      // We need trackers and info hash. 
      // Trackers we have in state. Info hash we don't directly have in the component unless we get it from refresh?
      // Wait, we can't easily get the info hash here unless we return it from the backend call.
      // The backend DOES return `ApiAddTorrentResponse` which has `id` and `details`. `details` has `info_hash`.
      // We need to capture that response.
  }

  // Refactor handleCreate to store the result
  const [createdTorrent, setCreatedTorrent] = useState<{info_hash: string, name: string} | null>(null);

  const handleCreate = async () => {
    setCreating(true);
    setError(null);
    setProgress(null);
    setCreatedTorrent(null);
    try {
      localStorage.setItem(STORAGE_KEY_TRACKERS, trackers);
      const trackerList = trackers
        .split(/[\n,]+/)
        .map((t) => t.trim())
        .filter((t) => t.length > 0);

      await API.createTorrentTask(path, {
        name: undefined, // user didn't specify name in modal explicitly
        trackers: trackerList
      });
      // Show success message or close
      onHide();
      // maybe trigger a toast or something?
      // but onHide close is fine.
    } catch (e: any) {
      console.error(e);
      let details = e;
      if (typeof e === "string") {
        details = { text: e };
      } else if (e instanceof Error) {
        details = { text: e.message };
      } else if (typeof e === "object" && e !== null && "human_readable" in e) {
        details = { ...e, text: e.human_readable };
      }
      setError({ text: "Error enqueueing torrent", details: details as any });
    } finally {
        setCreating(false);
    }
  };

  const copyCreatedMagnet = async () => {
    if (!createdTorrent) return;
    const trackerList = trackers
        .split(/[\n,]+/)
        .map((t) => t.trim())
        .filter((t) => t.length > 0);
    
    const magnet = generateMagnetLink(createdTorrent.info_hash, createdTorrent.name, trackerList);

    try {
        await navigator.clipboard.writeText(magnet);
    } catch (e) {
        console.error("Failed to copy", e);
    }
  };

  if (createdTorrent) {
      return (
        <Modal isOpen={true} onClose={onHide} title="Torrent Created">
            <ModalBody>
                <div className="flex flex-col items-center justify-center p-6 gap-4">
                    <div className="text-green-500 text-5xl">✓</div>
                    <div className="text-xl font-bold">Torrent Created Successfully!</div>
                    <div className="text-sm text-gray-500 dark:text-gray-400 break-all text-center">
                        {createdTorrent.name}
                    </div>
                    <div className="w-full">
                        <label className="block text-sm font-medium mb-1">Magnet Link</label>
                        <div className="flex gap-2">
                             <input 
                                readOnly 
                                value={`${generateMagnetLink(createdTorrent.info_hash, createdTorrent.name)}...`}
                                className="grow p-2 rounded border dark:bg-gray-800 dark:border-gray-700 text-sm text-gray-500"
                            />
                            <Button onClick={copyCreatedMagnet} variant="primary">Copy</Button>
                        </div>
                    </div>
                </div>
            </ModalBody>
            <ModalFooter>
                <Button onClick={onHide} variant="secondary">Close</Button>
            </ModalFooter>
        </Modal>
      );
  }

  return (
    <Modal isOpen={true} onClose={onHide} title="Create New Torrent">
      <ModalBody>
        <Form>
          <div className="flex gap-2 items-end">
            <div className="grow">
                <FormInput
                    label="Path"
                    name="path"
                    value={path}
                    onChange={(e) => setPath(e.target.value)}
                    placeholder="/path/to/files"
                    help="Absolute path to file or directory on the server"
                />
            </div>
            {isTauri.current && (
                <div className="flex gap-2 mb-4">
                    <Button onClick={() => handlePick(false)} variant="secondary">Add File</Button>
                    <Button onClick={() => handlePick(true)} variant="secondary">Add Dir</Button>
                </div>
            )}
          </div>
          <FormInput
            label="Trackers"
            name="trackers"
            value={trackers}
            onChange={(e) => setTrackers(e.target.value)}
            placeholder="udp://tracker.opentrackr.org:1337/announce"
            inputType="textarea"
          />
          <FormCheckbox
            label="Skip hash check (Server-side)"
            checked={skipHashCheck}
            onChange={() => setSkipHashCheck(!skipHashCheck)}
            help="Optimized creation: torrent is added immediately after creation without re-hashing."
            name="skip_hash_check"
            disabled={true}
          />
        </Form>
        <ErrorComponent error={error} />
        {progress && <ProgressBar current={progress.current} total={progress.total} />}
      </ModalBody>
      <ModalFooter>
        {creating && !progress && <Spinner />}
        <Button onClick={onHide} variant="cancel">
          Cancel
        </Button>
        <Button
          onClick={handleCreate}
          variant="primary"
          disabled={creating || !path}
        >
          Create (Queue)
        </Button>
      </ModalFooter>
    </Modal>
  );
};
