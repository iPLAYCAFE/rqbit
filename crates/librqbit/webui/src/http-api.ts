import {
  AddTorrentResponse,
  ErrorDetails,
  LimitsConfig,
  ListTorrentsResponse,
  PeerStatsSnapshot,
  RqbitAPI,
  SessionStats,
  TorrentDetails,
  TorrentStats,
  CreateTorrentTask,
} from "./api-types";
import { invoke } from "@tauri-apps/api/core";

// Define API URL and base path
const apiUrl = (() => {
  if (window.origin === "null") {
    return "http://localhost:3030";
  }

  const url = new URL(window.location.href);

  // assume Vite devserver
  if (url.port == "3031" || url.port == "1420") {
    return `${url.protocol}//${url.hostname}:3030`;
  }

  // Remove "/web" or "/web/" from the end and also ending slash.
  const path = /(.*?)\/?(\/web\/?)?$/.exec(url.pathname)![1] ?? "";
  return path;
})();

const makeBinaryRequest = async (path: string): Promise<ArrayBuffer> => {
  const url = apiUrl + path;
  const response = await fetch(url, {
    method: "GET",
    headers: {
      Accept: "application/octet-stream",
    },
  });

  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
  }

  return response.arrayBuffer();
};

const makeRequest = async (
  method: string,
  path: string,
  data?: any,
  isJson?: boolean,
): Promise<any> => {

  const url = apiUrl + path;
  let options: RequestInit = {
    method,
    headers: {
      Accept: "application/json",
    },
  };
  if (isJson) {
    options.headers = {
      Accept: "application/json",
      "Content-Type": "application/json",
    };
    options.body = JSON.stringify(data);
  } else {
    options.body = data;
  }

  let error: ErrorDetails = {
    method: method,
    path: path,
    text: "",
  };

  let response: Response;

  try {
    response = await fetch(url, options);
  } catch (e) {
    error.text = "network error";
    return Promise.reject(error);
  }

  error.status = response.status;
  error.statusText = `${response.status} ${response.statusText}`;

  if (!response.ok) {
    const errorBody = await response.text();
    try {
      const json = JSON.parse(errorBody);
      error.text =
        json.human_readable !== undefined
          ? json.human_readable
          : JSON.stringify(json, null, 2);
    } catch (e) {
      error.text = errorBody;
    }
    return Promise.reject(error);
  }
  const result = await response.json();
  return result;
};

// Helper to check if we are in Tauri
const isTauri = () => "__TAURI_INTERNALS__" in window;

export const API: RqbitAPI & { getVersion: () => Promise<string> } = {
  getStreamLogsUrl: () => isTauri() ? null : apiUrl + "/stream_logs",
  
  listTorrents: (opts?: {
    withStats?: boolean;
  }): Promise<ListTorrentsResponse> => {
    if (isTauri()) {
        return invoke("torrents_list", { opts });
    }
    const url = opts?.withStats ? "/torrents?with_stats=true" : "/torrents";
    return makeRequest("GET", url);
  },
  
  getTorrentDetails: (index: number): Promise<TorrentDetails> => {
    if (isTauri()) {
        return invoke("torrent_details", { id: index });
    }
    return makeRequest("GET", `/torrents/${index}`);
  },
  
  getTorrentStats: (index: number): Promise<TorrentStats> => {
    if (isTauri()) {
        return invoke("torrent_stats", { id: index });
    }
    return makeRequest("GET", `/torrents/${index}/stats/v1`);
  },
  
  getPeerStats: (index: number): Promise<PeerStatsSnapshot> => {
    if (isTauri()) {
        return invoke("torrent_peer_stats", { id: index, filter: "live" }); // Assuming filter 'live' matches 'state=live'
    }
    return makeRequest("GET", `/torrents/${index}/peer_stats?state=live`);
  },
  
  stats: (): Promise<SessionStats> => {
    if (isTauri()) {
        return invoke("stats");
    }
    return makeRequest("GET", "/stats");
  },

  uploadTorrent: (data, opts): Promise<AddTorrentResponse> => {
    if (isTauri()) {
        // Warning: This simplified bridge handles the most common case (file path or magnet).
        // For actual file upload from browser JS in Tauri, we might need 'torrent_create_from_base64_file' 
        // or 'torrent_create_from_url' depending on input.
        // Assuming 'data' is URL string for magnet/http.
        if (typeof data === "string") {
            return invoke("torrent_create_from_url", { url: data, opts });
        }
        // If data is File/Blob, we might need arrayBuffer -> base64
        // For now, falling back to HTTP if complex? Or implementing base64.
        // Let's implement base64 for File.
        if ((data as any) instanceof File || (data as any) instanceof Blob) {
             return new Promise((resolve, reject) => {
                const reader = new FileReader();
                reader.onload = () => {
                   const base64 = (reader.result as string).split(',')[1];
                   invoke("torrent_create_from_base64_file", { contents: base64, opts })
                      .then(r => resolve(r as AddTorrentResponse))
                      .catch(reject);
                };
                reader.onerror = reject;
                reader.readAsDataURL(data);
             });
        }
    }

    let url = "/torrents?";
    if (opts?.overwrite ?? true) {
      url += "&overwrite=true";
    }
    if (opts?.list_only) {
      url += "&list_only=true";
    }
    if (opts?.only_files != null) {
      url += `&only_files=${opts.only_files.join(",")}`;
    }
    if (opts?.peer_opts?.connect_timeout) {
      url += `&peer_connect_timeout=${opts.peer_opts.connect_timeout}`;
    }
    if (opts?.peer_opts?.read_write_timeout) {
      url += `&peer_read_write_timeout=${opts.peer_opts.read_write_timeout}`;
    }
    if (opts?.initial_peers) {
      url += `&initial_peers=${opts.initial_peers.join(",")}`;
    }
    if (opts?.output_folder) {
      url += `&output_folder=${opts.output_folder}`;
    }
    if (opts?.skip_initial_check) {
      url += `&skip_initial_check=true`;
    }
    if (opts?.sync_extra_files) {
      url += `&sync_extra_files=true`;
    }
    if (typeof data === "string") {
      url += "&is_url=true";
    }
    return makeRequest("POST", url, data);
  },

  createTorrent: (
    path: string,
    opts?: {
      name?: string;
      trackers?: string[];
    },
  ): Promise<AddTorrentResponse> => {
    if (isTauri()) {
        return invoke("torrent_create", { path, name: opts?.name, trackers: opts?.trackers });
    }
    let url = `/torrents/create?path=${encodeURIComponent(path)}`;
    if (opts?.name) {
      url += `&name=${encodeURIComponent(opts.name)}`;
    }
    if (opts?.trackers) {
      opts.trackers.forEach(t => url += `&trackers=${encodeURIComponent(t)}`);
    }
    return makeRequest("POST", url);
  },

  updateOnlyFiles: (index: number, files: number[]): Promise<void> => {
    if (isTauri()) {
        return invoke("torrent_action_configure", { id: index, onlyFiles: files });
    }
    let url = `/torrents/${index}/update_only_files`;
    return makeRequest(
      "POST",
      url,
      {
        only_files: files,
      },
      true,
    );
  },

  pause: (index: number): Promise<void> => {
    if (isTauri()) { return invoke("torrent_action_pause", { id: index }); }
    return makeRequest("POST", `/torrents/${index}/pause`);
  },

  start: (index: number): Promise<void> => {
    if (isTauri()) { return invoke("torrent_action_start", { id: index }); }
    return makeRequest("POST", `/torrents/${index}/start`);
  },

  forget: (index: number): Promise<void> => {
    if (isTauri()) { return invoke("torrent_action_forget", { id: index }); }
    return makeRequest("POST", `/torrents/${index}/forget`);
  },

  delete: (index: number): Promise<void> => {
    if (isTauri()) { return invoke("torrent_action_delete", { id: index }); }
    return makeRequest("POST", `/torrents/${index}/delete`);
  },
  
  getVersion: async (): Promise<string> => {
    if (isTauri()) { return invoke("get_version"); }
    const r = await makeRequest("GET", "/");
    return r.version;
  },
  
  getTorrentStreamUrl: (
    index: number,
    file_id: number,
    filename?: string | null,
  ) => {
    // Stream URL still needs HTTP usually, unless we have a custom protocol handler for streaming.
    // rqbit likely streams via HTTP.
    // If we are in Tauri, we might need to point to localhost:3030 if the server is running.
    // Or usage of 'asset://' ? 
    // For now, keeping as is, assuming apiUrl will point to localhost if needed.
    // However, if apiUrl is faulty (tauri://), streaming won't work.
    // TODO: Verify streaming in Tauri.
    let url = apiUrl + `/torrents/${index}/stream/${file_id}`;
    if (!!filename) {
      url += `/${filename}`;
    }
    return url;
  },
  
  getPlaylistUrl: (index: number) => {
    return (apiUrl || window.origin) + `/torrents/${index}/playlist`;
  },
  
  getTorrentHaves: async (index: number): Promise<Uint8Array> => {
    if (isTauri()) {
        // Tauri returns integer array (byte values), we need Uint8Array.
        // The command 'torrent_haves' returns Raw.
        const res = await invoke<number[]>("torrent_haves", { id: index });
        return new Uint8Array(res);
    }
    return new Uint8Array(await makeBinaryRequest(`/torrents/${index}/haves`));
  },
  
  getLimits: (): Promise<LimitsConfig> => {
    if (isTauri()) {
        return invoke("get_limits");
    }
    return makeRequest("GET", "/torrents/limits");
  },
  
  setLimits: (limits: LimitsConfig): Promise<void> => {
    if (isTauri()) {
        return invoke("set_limits", { limits });
    }
    return makeRequest("POST", "/torrents/limits", limits, true);
  },

  createTorrentTask: (
    path: string,
    opts?: { name?: string; trackers?: string[] },
  ): Promise<number> => {
    if (isTauri()) {
        return invoke<{id: number}>("torrent_create_task_enqueue", {
            path,
            name: opts?.name,
            trackers: opts?.trackers
        }).then(r => r.id);
    }
    let url = `/torrents/create_task?path=${encodeURIComponent(path)}`;
    if (opts?.name) {
      url += `&name=${encodeURIComponent(opts.name)}`;
    }
    if (opts?.trackers) {
      opts.trackers.forEach(t => url += `&trackers=${encodeURIComponent(t)}`);
    }
    return makeRequest("POST", url).then(r => r.id);
  },

  listCreateTorrentTasks: (): Promise<CreateTorrentTask[]> => {
    if (isTauri()) {
        return invoke("torrent_create_task_list");
    }
    return makeRequest("GET", "/torrents/create_tasks");
  },

  cancelCreateTorrentTask: (id: number): Promise<void> => {
    if (isTauri()) {
        return invoke("torrent_create_task_cancel", { id });
    }
    return makeRequest("DELETE", `/torrents/create_tasks/${id}`);
  },

  deleteCreateTorrentTask: (id: number): Promise<void> => {
    if (isTauri()) {
        return invoke("torrent_create_task_delete", { id });
    }
    return makeRequest("DELETE", `/torrents/create_tasks/${id}/delete`);
  },

  listExtraFiles: (index: number): Promise<{ extra_files: string[] }> => {
    if (isTauri()) {
        return invoke("torrent_list_extra_files", { id: index });
    }
    return makeRequest("GET", `/torrents/${index}/extra_files`);
  },

  removeExtraFiles: (
    index: number,
    files: string[],
  ): Promise<{ removed: number; failed: number }> => {
    if (isTauri()) {
        return invoke("torrent_delete_extra_files", { id: index, files });
    }
    return makeRequest("POST", `/torrents/${index}/delete_extra_files`, { files }, true);
  },
};
