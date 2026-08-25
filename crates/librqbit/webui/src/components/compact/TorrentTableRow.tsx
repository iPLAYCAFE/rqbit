import { TorrentListItem, STATE_INITIALIZING } from "../../api-types";
import { StatusIcon } from "../StatusIcon";
import { formatBytes } from "../../helper/formatBytes";
import { formatRelativeTime } from "../../helper/formatDate";
import { getCompletionETA } from "../../helper/getCompletionETA";

import { memo } from "react";

import { IconButton } from "../buttons/IconButton";
import { CopyMagnetButton } from "../CopyMagnetButton";

interface TorrentTableRowProps {
  torrent: TorrentListItem;
  isSelected: boolean;
  onRowClick: (id: number, e: React.MouseEvent) => void;
  onCheckboxChange: (id: number) => void;
}

const TorrentTableRowUnmemoized: React.FC<TorrentTableRowProps> = ({
  torrent,
  isSelected,
  onRowClick,
  onCheckboxChange,
}) => {
  const stats = torrent.stats;
  const state = stats?.state ?? "";
  const error = stats?.error ?? null;
  const totalBytes = stats?.total_bytes ?? 1;
  const progressBytes = stats?.progress_bytes ?? 0;
  const finished = stats?.finished || false;
  const live = !!stats?.live;

  const progressPercentage = error
    ? 100
    : totalBytes === 0
      ? 100
      : Math.round((progressBytes / totalBytes) * 100);

  const downloadSpeed = stats?.live?.download_speed?.human_readable ?? "-";
  const uploadSpeed = stats?.live?.upload_speed?.human_readable ?? "-";
  const uploadedBytes = stats?.live?.snapshot.uploaded_bytes ?? 0;

  const peerStats = stats?.live?.snapshot.peer_stats;
  const peersDisplay = peerStats ? `${peerStats.live}/${peerStats.seen}` : "-";

  const eta = stats ? getCompletionETA(stats) : "-";
  const displayEta = finished ? "Done" : eta;

  const name = torrent.name ?? "";
  const id = torrent.id;

  const fetchedBytes = stats?.total_fetched_bytes ?? 0;
  const remainingBytes = Math.max(0, totalBytes - progressBytes);
  const lastActive = stats?.last_activity ?? null;
  const addedAt = stats?.added_at ?? null;

  const handleRowClick = (e: React.MouseEvent) => {
    onRowClick(torrent.id, e);
  };

  const handleCheckboxClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onCheckboxChange(torrent.id);
  };

  // Common cell styles to avoid repetition
  const cellBase = "px-2 flex items-center shrink-0";
  const numericCell = `w-20 ${cellBase} justify-end text-right text-secondary whitespace-nowrap`;
  const centeredCell = `w-20 ${cellBase} justify-center text-center text-secondary whitespace-nowrap`;

  return (
    <div
      onMouseDown={handleRowClick}
      className={`group cursor-pointer border-b border-divider text-sm h-8 flex items-center ${
        isSelected ? "bg-primary/10" : "hover:bg-surface-raised"
      }`}
    >
      <div
        className={`w-8 ${cellBase} justify-center`}
        onMouseDown={handleCheckboxClick}
      >
        <input
          type="checkbox"
          checked={isSelected}
          onChange={() => {}}
          className="w-4 h-4 rounded border-divider-strong bg-surface text-primary focus:ring-primary"
        />
      </div>
      <div className="w-8 px-1 flex items-center justify-center shrink-0">
        <StatusIcon
          className="w-5 h-5"
          error={!!error}
          live={live}
          finished={finished}
        />
      </div>
      <div
        className={`w-12 ${cellBase} justify-center text-center text-tertiary font-mono whitespace-nowrap`}
      >
        {torrent.id}
      </div>
      <div className="flex-1 min-w-0 px-2 flex flex-col justify-center">
        <div className="flex items-center justify-between gap-2 overflow-hidden">
          <div className="truncate" title={name}>
            {name || "Loading..."}
          </div>
          <div className="opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
            <CopyMagnetButton torrent={torrent} iconClassName="w-3 h-3" />
          </div>
        </div>
        {error && (
          <div className="truncate text-xs text-error" title={error}>
            {error}
          </div>
        )}
      </div>
      <div className={centeredCell} title={addedAt || ""}>{formatRelativeTime(addedAt)}</div>
      <div className={numericCell}>{formatBytes(totalBytes)}</div>
      <div className={`w-24 ${cellBase} justify-center`}>
        <div className="flex items-center gap-2 w-full">
          <div className="flex-1 h-1.5 bg-divider rounded-full overflow-hidden">
            <div
              className={`h-full rounded-full ${
                error
                  ? "bg-error-bg"
                  : finished
                    ? "bg-success-bg"
                    : state === STATE_INITIALIZING
                      ? "bg-warning-bg"
                      : "bg-primary-bg"
              }`}
              style={{ width: `${progressPercentage}%` }}
            />
          </div>
          <span className="text-sm text-secondary w-8 text-right shrink-0">
            {progressPercentage}%
          </span>
        </div>
      </div>
      <div className={numericCell}>{formatBytes(fetchedBytes)}</div>
      <div className={numericCell}>{formatBytes(remainingBytes)}</div>
      <div className={numericCell}>{downloadSpeed}</div>
      <div className={numericCell}>{uploadSpeed}</div>
      <div className={numericCell}>
        {uploadedBytes > 0 && <>{formatBytes(uploadedBytes)}</>}
      </div>
      <div className={`${numericCell} text-xs`} title={lastActive || ""}>{formatRelativeTime(lastActive)}</div>
      <div className={centeredCell}>{displayEta}</div>
      <div
        className={`w-16 ${cellBase} justify-center text-center text-secondary whitespace-nowrap`}
      >
        {peersDisplay}
      </div>
    </div>
  );
};

export const TorrentTableRow = memo(TorrentTableRowUnmemoized);
