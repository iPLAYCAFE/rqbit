import { useContext, useEffect, useState } from "react";
import { TorrentDetails, TorrentStats, ErrorDetails } from "../../api-types";
import { APIContext } from "../../context";
import { FileListInput } from "../FileListInput";
import { useErrorStore } from "../../stores/errorStore";
import { CleanExtraFilesModal } from "../modal/CleanExtraFilesModal";
import { FaCheckDouble } from "react-icons/fa";

interface FilesTabProps {
  torrentId: number;
  detailsResponse: TorrentDetails | null;
  statsResponse: TorrentStats | null;
  onRefresh?: () => void;
}

export const FilesTab: React.FC<FilesTabProps> = ({
  torrentId,
  detailsResponse,
  statsResponse,
  onRefresh,
}) => {
  const [selectedFiles, setSelectedFiles] = useState<Set<number>>(new Set());
  const [savingSelectedFiles, setSavingSelectedFiles] = useState(false);
  const [showCleanExtraFiles, setShowCleanExtraFiles] = useState(false);

  const API = useContext(APIContext);
  const setCloseableError = useErrorStore((state) => state.setCloseableError);

  useEffect(() => {
    setSelectedFiles(
      new Set<number>(
        detailsResponse?.files
          .map((f, id) => ({ f, id }))
          .filter(({ f }) => f.included)
          .map(({ id }) => id) ?? [],
      ),
    );
  }, [detailsResponse]);

  const updateSelectedFiles = (selectedFiles: Set<number>) => {
    setSavingSelectedFiles(true);
    API.updateOnlyFiles(torrentId, Array.from(selectedFiles))
      .then(
        () => {
          onRefresh?.();
          setCloseableError(null);
        },
        (e) => {
          setCloseableError({
            text: "Error configuring torrent",
            details: e as ErrorDetails,
          });
        },
      )
      .finally(() => setSavingSelectedFiles(false));
  };

  if (!detailsResponse) {
    return <div className="p-4 text-tertiary">Loading...</div>;
  }

  return (
    <div className="p-2 text-sm">
      <div className="flex items-center justify-between mb-2">
        <span className="text-tertiary text-xs">
          Remove files not in the torrent manifest
        </span>
        <button
          className="flex items-center gap-1.5 px-3 py-1 text-xs font-medium rounded
            bg-surface-raised border border-divider text-secondary
            hover:bg-accent/10 hover:text-accent hover:border-accent/30
            transition-colors"
          onClick={() => setShowCleanExtraFiles(true)}
        >
          <FaCheckDouble className="text-[10px]" />
          Clean Extra Files
        </button>
      </div>
      <FileListInput
        torrentId={torrentId}
        torrentDetails={detailsResponse}
        torrentStats={statsResponse}
        selectedFiles={selectedFiles}
        setSelectedFiles={updateSelectedFiles}
        disabled={savingSelectedFiles}
        allowStream
        showProgressBar
      />
      <CleanExtraFilesModal
        show={showCleanExtraFiles}
        onHide={() => setShowCleanExtraFiles(false)}
        torrentId={torrentId}
        torrentName={detailsResponse.name ?? null}
      />
    </div>
  );
};
