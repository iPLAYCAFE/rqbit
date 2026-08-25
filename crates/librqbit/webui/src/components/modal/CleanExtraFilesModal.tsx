import { useContext, useEffect, useState } from "react";
import { APIContext } from "../../context";
import { ErrorWithLabel } from "../../rqbit-web";
import { Button } from "../buttons/Button";
import { ErrorComponent } from "../ErrorComponent";
import { Spinner } from "../Spinner";
import { Modal } from "./Modal";
import { ModalBody } from "./ModalBody";
import { ModalFooter } from "./ModalFooter";

export const CleanExtraFilesModal: React.FC<{
    show: boolean;
    onHide: () => void;
    torrentId: number;
    torrentName: string | null;
}> = ({ show, onHide, torrentId, torrentName }) => {
    const [files, setFiles] = useState<string[]>([]);
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [loading, setLoading] = useState(false);
    const [cleaning, setCleaning] = useState(false);
    const [error, setError] = useState<ErrorWithLabel | null>(null);
    const [result, setResult] = useState<{
        removed: number;
        failed: number;
    } | null>(null);

    const API = useContext(APIContext);

    // Fetch extra files when modal opens
    useEffect(() => {
        if (!show) return;
        setLoading(true);
        setError(null);
        setResult(null);
        setFiles([]);
        setSelected(new Set());

        API.listExtraFiles(torrentId).then(
            (res) => {
                setFiles(res.extra_files);
                setSelected(new Set(res.extra_files));
                setLoading(false);
            },
            (e) => {
                setError({ text: "Failed to list extra files", details: e });
                setLoading(false);
            },
        );
    }, [show, torrentId]);

    if (!show) return null;

    const close = () => {
        setError(null);
        setResult(null);
        setCleaning(false);
        onHide();
    };

    const toggleFile = (file: string) => {
        setSelected((prev) => {
            const next = new Set(prev);
            if (next.has(file)) {
                next.delete(file);
            } else {
                next.add(file);
            }
            return next;
        });
    };

    const toggleAll = () => {
        if (selected.size === files.length) {
            setSelected(new Set());
        } else {
            setSelected(new Set(files));
        }
    };

    const clean = async () => {
        setCleaning(true);
        setError(null);
        try {
            const res = await API.removeExtraFiles(torrentId, [...selected]);
            setResult(res);
            if (res.failed === 0) {
                // Remove cleaned files from the list
                setFiles((prev) => prev.filter((f) => !selected.has(f)));
                setSelected(new Set());
            }
        } catch (e: any) {
            setError({ text: "Failed to remove extra files", details: e });
        } finally {
            setCleaning(false);
        }
    };

    const title = `Clean Extra Files - ${torrentName || `Torrent #${torrentId}`}`;

    return (
        <Modal isOpen={show} onClose={close} title={title}>
            <ModalBody>
                {loading && (
                    <div className="flex items-center justify-center py-8">
                        <Spinner />
                        <span className="ml-2 text-secondary">
                            Scanning for extra files...
                        </span>
                    </div>
                )}

                {!loading && files.length === 0 && !error && (
                    <p className="text-secondary py-4 text-center">
                        No extra files found. The directory is clean.
                    </p>
                )}

                {!loading && files.length > 0 && (
                    <>
                        <p className="text-secondary mb-3">
                            {files.length} extra file{files.length > 1 ? "s" : ""} found in
                            the torrent directory. Select files to remove:
                        </p>

                        <div className="mb-2">
                            <label className="flex items-center gap-2 cursor-pointer text-sm text-secondary">
                                <input
                                    type="checkbox"
                                    className="form-checkbox h-4 w-4 text-blue-500 rounded border-gray-300 dark:border-slate-600"
                                    checked={selected.size === files.length}
                                    onChange={toggleAll}
                                />
                                Select all
                            </label>
                        </div>

                        <div className="rounded-md bg-gray-50 dark:bg-slate-700/50 p-3 max-h-64 overflow-y-auto">
                            <ul className="space-y-1">
                                {files.map((file) => (
                                    <li key={file} className="flex items-center gap-2">
                                        <input
                                            type="checkbox"
                                            className="form-checkbox h-4 w-4 text-blue-500 rounded border-gray-300 dark:border-slate-600 flex-shrink-0"
                                            checked={selected.has(file)}
                                            onChange={() => toggleFile(file)}
                                        />
                                        <span
                                            className="text-sm text-text truncate"
                                            title={file}
                                        >
                                            {file}
                                        </span>
                                    </li>
                                ))}
                            </ul>
                        </div>
                    </>
                )}

                {result && (
                    <div
                        className={`mt-3 p-2 rounded text-sm ${result.failed > 0
                            ? "bg-red-50 text-red-700 dark:bg-red-900/30 dark:text-red-300"
                            : "bg-green-50 text-green-700 dark:bg-green-900/30 dark:text-green-300"
                            }`}
                    >
                        Removed {result.removed} file{result.removed !== 1 ? "s" : ""}
                        {result.failed > 0 && ` (${result.failed} failed)`}.
                    </div>
                )}

                {error && <ErrorComponent error={error} />}
            </ModalBody>

            <ModalFooter>
                {cleaning && <Spinner />}
                <Button variant="cancel" onClick={close}>
                    {files.length === 0 && !loading ? "Close" : "Cancel"}
                </Button>
                {files.length > 0 && (
                    <Button
                        variant="danger"
                        onClick={clean}
                        disabled={cleaning || selected.size === 0}
                    >
                        Remove {selected.size} File{selected.size !== 1 ? "s" : ""}
                    </Button>
                )}
            </ModalFooter>
        </Modal>
    );
};
