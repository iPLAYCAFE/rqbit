
import { useContext, useEffect, useState } from "react";
import { APIContext } from "../../context";
import { CreateTorrentTask } from "../../api-types";
import { formatBytes } from "../../helper/formatBytes";
import { formatRelativeTime } from "../../helper/formatDate";
import { Modal } from "./Modal";
import { ModalBody } from "./ModalBody";
import { ModalFooter } from "./ModalFooter";
import { Button } from "../buttons/Button";
import { Spinner } from "../Spinner";
import { IconButton } from "../buttons/IconButton";
import { CopyMagnetButton } from "../CopyMagnetButton";
import { FiTrash2 } from "react-icons/fi";

export const CreateTorrentQueueModal = (props: { onHide: () => void }) => {
  const { onHide } = props;
  const API = useContext(APIContext);
  const [tasks, setTasks] = useState<CreateTorrentTask[] | null>(null);
  const [loading, setLoading] = useState(false);

  const fetchTasks = async () => {
    try {
      const list = await API.listCreateTorrentTasks();
      // Sort by id ascending (oldest first)
      list.sort((a, b) => a.id - b.id);
      setTasks(list);
    } catch (e) {
      console.error("Error fetching tasks", e);
    }
  };

  useEffect(() => {
    setLoading(true);
    fetchTasks().then(() => setLoading(false));
    const interval = setInterval(fetchTasks, 1000);
    return () => clearInterval(interval);
  }, []);

  const [confirmCancelId, setConfirmCancelId] = useState<number | null>(null);

  const handleCancelClick = (id: number) => {
    setConfirmCancelId(id);
    // Auto-reset confirmation after 3 seconds if not clicked
    setTimeout(() => {
        setConfirmCancelId(prev => prev === id ? null : prev);
    }, 3000);
  };

  const handleConfirmCancel = async (id: number) => {
    try {
      setConfirmCancelId(null);
      await API.cancelCreateTorrentTask(id);
      await fetchTasks();
    } catch (e) {
      console.error("Error cancelling task", e);
    }
  };

  return (
    <Modal isOpen={true} onClose={onHide} title="Torrent Creation Queue">
      <ModalBody>
        <div className="flex flex-col gap-2 min-h-[300px] max-h-[60vh] overflow-y-auto">
          {loading && !tasks && (
            <div className="flex justify-center items-center h-full">
              <Spinner />
            </div>
          )}
          {tasks && tasks.length === 0 && (
            <div className="text-center text-gray-500 py-8">No tasks in queue</div>
          )}
          {tasks &&
            tasks.map((task) => (
              <div
                key={task.id}
                className="bg-surface-raised border border-divider rounded p-3 text-sm flex flex-col gap-1"
              >
                <div className="flex justify-between items-start">
                  <div className="font-mono text-xs text-tertiary">#{task.id}</div>
                  <div className="flex items-center gap-2">
                     <span className={`text-xs px-2 py-0.5 rounded-full ${
                        task.status === "processing" ? "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200" :
                        task.status === "done" ? "bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200" :
                        task.status === "pending" ? "bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300" :
                        "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200"
                     }`}>
                         {task.status.toUpperCase()}
                     </span>
                     {task.magnet_link && (
                        <div className="flex items-center">
                            <CopyMagnetButton 
                                torrent={{ info_hash: "", name: task.source_path } as any} 
                                magnetLinkOverride={task.magnet_link}
                                iconClassName="w-4 h-4" 
                            />
                        </div>
                     )}
                     {(task.status === "pending" || task.status === "processing") ? (
                        confirmCancelId === task.id ? (
                            <Button
                                onClick={() => handleConfirmCancel(task.id)}
                                variant="danger"
                                size="sm"
                                className="animate-pulse"
                            >
                                Confirm
                            </Button>
                        ) : (
                            <IconButton
                                onClick={() => handleCancelClick(task.id)}
                                className="text-error hover:bg-error/10"
                                title="Cancel"
                            >
                                <FiTrash2 />
                            </IconButton>
                        )
                     ) : (
                        <IconButton
                            onClick={async () => {
                                try {
                                    await API.deleteCreateTorrentTask(task.id);
                                    await fetchTasks();
                                } catch (e) {
                                    console.error(e);
                                }
                            }}
                            className="text-tertiary hover:bg-surface-highlight"
                            title="Remove from list"
                        >
                            <FiTrash2 />
                        </IconButton>
                     )}
                  </div>
                </div>
                <div className="truncate font-medium" title={task.source_path}>
                   {task.source_path}
                </div>
                <div className="flex justify-between text-xs text-secondary">
                    <div>{formatRelativeTime(task.created_at)}</div>
                    <div>
                        {formatBytes(task.processed_bytes)} / {formatBytes(task.total_bytes)}
                    </div>
                </div>
                {task.status === "processing" && (
                    <div className="w-full bg-gray-200 rounded-full h-1.5 dark:bg-gray-700 mt-1">
                      <div
                        className="bg-primary h-1.5 rounded-full transition-all duration-300"
                        style={{ width: `${task.total_bytes > 0 ? (task.processed_bytes / task.total_bytes) * 100 : 0}%` }}
                      ></div>
                    </div>
                )}
                {task.error && (
                    <div className="text-error text-xs mt-1 break-all">{task.error}</div>
                )}
              </div>
            ))}
        </div>
      </ModalBody>
      <ModalFooter>
        <Button onClick={onHide} variant="secondary">
          Close
        </Button>
      </ModalFooter>
    </Modal>
  );
};
