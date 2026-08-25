import { useContext, useEffect, useState } from "react";
import { AddTorrentResponse, AddTorrentOptions } from "../../api-types";
import { APIContext } from "../../context";
import { ErrorComponent } from "../ErrorComponent";
import { ErrorWithLabel } from "../../rqbit-web";
import { Spinner } from "../Spinner";
import { Modal } from "./Modal";
import { ModalBody } from "./ModalBody";
import { ModalFooter } from "./ModalFooter";
import { Button } from "../buttons/Button";
import { Fieldset } from "../forms/Fieldset";
import { FormCheckbox } from "../forms/FormCheckbox";
import { FormInput } from "../forms/FormInput";
import { Form } from "../forms/Form";
import { FileListInput } from "../FileListInput";
import { useTorrentStore } from "../../stores/torrentStore";

const RECENT_FOLDER_KEY = "rqbit_recent_output_parent";
const SYNC_EXTRA_FILES_KEY = "rqbit_sync_extra_files";

/** Detect path separator style (backslash for Windows paths, forward slash otherwise) */
function detectSep(path: string): string {
  return path.includes("\\") ? "\\" : "/";
}

/** Normalize all separators in a path to the detected style */
function normalizeSeps(path: string): string {
  const sep = detectSep(path);
  if (sep === "\\") {
    return path.replace(/\//g, "\\");
  }
  return path.replace(/\\/g, "/");
}

/** Get parent directory of a path (preserves original separator style) */
function getParentDir(path: string): string {
  const normalized = normalizeSeps(path).replace(/[/\\]+$/, "");
  const sep = detectSep(normalized);
  const lastSlash = normalized.lastIndexOf(sep);
  if (lastSlash <= 0) return normalized;
  return normalized.substring(0, lastSlash);
}

/** Get the last component (torrent name) from a path */
function getBaseName(path: string): string {
  const normalized = normalizeSeps(path).replace(/[/\\]+$/, "");
  const sep = detectSep(normalized);
  const lastSlash = normalized.lastIndexOf(sep);
  if (lastSlash < 0) return normalized;
  return normalized.substring(lastSlash + 1);
}

/** Reconstruct path using the original separator style */
function joinPath(parent: string, name: string, originalPath: string): string {
  const sep = detectSep(originalPath);
  const trimmed = normalizeSeps(parent).replace(/[/\\]+$/, "");
  return trimmed + sep + name;

}

export const FileSelectionModal = (props: {
  onHide: () => void;
  listTorrentResponse: AddTorrentResponse | null;
  listTorrentError: ErrorWithLabel | null;
  listTorrentLoading: boolean;
  data: string | File;
}) => {
  let {
    onHide,
    listTorrentResponse,
    listTorrentError,
    listTorrentLoading,
    data,
  } = props;

  const [selectedFiles, setSelectedFiles] = useState<Set<number>>(new Set());
  const [uploading, setUploading] = useState(false);
  const [uploadError, setUploadError] = useState<ErrorWithLabel | null>(null);
  const [unpopularTorrent, setUnpopularTorrent] = useState(false);
  const [skipInitialCheck, setSkipInitialCheck] = useState(false);
  const [syncExtraFiles, setSyncExtraFiles] = useState(() => {
    return localStorage.getItem(SYNC_EXTRA_FILES_KEY) === "true";
  });
  const [outputFolder, setOutputFolder] = useState<string>("");
  const [overwrite, setOverwrite] = useState(true);
  const refreshTorrents = useTorrentStore((state) => state.refreshTorrents);
  const API = useContext(APIContext);

  useEffect(() => {
    setSelectedFiles(
      new Set(
        listTorrentResponse?.details.files.flatMap((file, idx) => {
          if (file.attributes.padding) {
            return [];
          } else {
            return [idx];
          }
        }),
      ),
    );

    if (listTorrentResponse?.output_folder) {
      const defaultPath = listTorrentResponse.output_folder;
      const torrentName = getBaseName(defaultPath);
      const savedParent = localStorage.getItem(RECENT_FOLDER_KEY);

      if (savedParent && torrentName) {
        // Use the saved recent parent + current torrent name
        setOutputFolder(joinPath(savedParent, torrentName, defaultPath));
      } else {
        setOutputFolder(defaultPath);
      }
    } else {
      setOutputFolder("");
    }
  }, [listTorrentResponse]);

  const clear = () => {
    onHide();
    setSelectedFiles(new Set());
    setUploadError(null);
    setUploading(false);
  };

  const handleUpload = async () => {
    if (!listTorrentResponse) {
      return;
    }
    setUploading(true);
    let initialPeers = listTorrentResponse.seen_peers
      ? listTorrentResponse.seen_peers.slice(0, 32)
      : null;

    // Determine if all non-padding files are selected;
    // if so, omit only_files to avoid 414 URI Too Long for large torrents.
    const allNonPaddingFiles = new Set(
      listTorrentResponse.details.files.flatMap((file, idx) =>
        file.attributes.padding ? [] : [idx],
      ),
    );
    const allSelected =
      selectedFiles.size === allNonPaddingFiles.size &&
      [...selectedFiles].every((f) => allNonPaddingFiles.has(f));

    let opts: AddTorrentOptions = {
      overwrite: overwrite,
      only_files: allSelected ? undefined : Array.from(selectedFiles),
      initial_peers: initialPeers,
      output_folder: outputFolder,
      skip_initial_check: skipInitialCheck,
      sync_extra_files: syncExtraFiles || undefined,
    };

    if (unpopularTorrent) {
      opts.peer_opts = {
        connect_timeout: 20,
        read_write_timeout: 60,
      };
    }
    API.uploadTorrent(data, opts)
      .then(
        (response) => {
          if (response?.already_managed) {
            const name = response.details?.name || response.output_folder || "Unknown";
            alert(`⚠️ Torrent "${name}" is already managed.\n\nOutput folder: ${response.output_folder}`);
          }

          // Save the parent directory of the output folder for next time
          if (outputFolder) {
            const parent = getParentDir(outputFolder);
            if (parent) {
              localStorage.setItem(RECENT_FOLDER_KEY, parent);
            }
          }

          onHide();
          refreshTorrents();
        },
        (e) => {
          setUploadError({ text: "Error starting torrent", details: e });
        },
      )
      .finally(() => setUploading(false));
  };

  const getBody = () => {
    if (listTorrentLoading) {
      return <Spinner label="Loading torrent contents" />;
    } else if (listTorrentError) {
      return <ErrorComponent error={listTorrentError}></ErrorComponent>;
    } else if (listTorrentResponse) {
      return (
        <Form>
          <FormInput
            label="Output folder"
            name="output_folder"
            inputType="text"
            value={outputFolder}
            onChange={(e) => setOutputFolder(e.target.value)}
          />

          <Fieldset>
            <FileListInput
              selectedFiles={selectedFiles}
              setSelectedFiles={setSelectedFiles}
              torrentDetails={listTorrentResponse.details}
              torrentStats={null}
            />
          </Fieldset>

          <Fieldset label="Options">
            <FormCheckbox
              label="Overwrite existing files"
              checked={overwrite}
              onChange={() => setOverwrite(!overwrite)}
              help="Allow writing to existing files (required for resuming)"
              name="overwrite"
            />
            <FormCheckbox
              label="Skip hash check"
              checked={skipInitialCheck}
              onChange={() => setSkipInitialCheck(!skipInitialCheck)}
              help="Trust that existing files are correct. Useful for large torrents."
              name="skip_initial_check"
            />
            <FormCheckbox
              label="Auto-delete extra files"
              checked={syncExtraFiles}
              onChange={() => {
                const newVal = !syncExtraFiles;
                setSyncExtraFiles(newVal);
                localStorage.setItem(SYNC_EXTRA_FILES_KEY, String(newVal));
              }}
              help="Automatically remove files not in the torrent after download completes."
              name="sync_extra_files"
            />
          </Fieldset>
        </Form>
      );
    }
  };
  return (
    <Modal isOpen={true} onClose={clear} title="Add Torrent">
      <ModalBody>
        {getBody()}
        <ErrorComponent error={uploadError} />
      </ModalBody>
      <ModalFooter>
        {uploading && <Spinner />}
        <Button onClick={clear} variant="cancel">
          Cancel
        </Button>
        <Button
          onClick={handleUpload}
          variant="primary"
          disabled={listTorrentLoading || uploading || selectedFiles.size == 0}
        >
          OK
        </Button>
      </ModalFooter>
    </Modal>
  );
};
