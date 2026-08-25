import { useContext } from "react";
import { APIContext } from "../../context";
import { ErrorComponent } from "../ErrorComponent";
import { LogStream } from "../LogStream";
import { Modal } from "./Modal";
import { ModalFooter } from "./ModalFooter";
import { ModalBody } from "./ModalBody";
import { Button } from "../buttons/Button";

interface Props {
  show: boolean;
  onClose: () => void;
}



export const LogStreamModal: React.FC<Props> = ({ show, onClose }) => {
  const api = useContext(APIContext);
  let logsUrl = api.getStreamLogsUrl();


  // We reuse LogStream component logic? No, LogStream is tied to fetch.
  // We should prob make LogStream accept external lines or provider.
  // But for speed, let's just make a TauriLogStream component that renders lines similarly.
  
  // Actually, LogStream component does too much UI work to duplicate. 
  // Better to modify LogStream to accept "mode" or just pass "tauri" as url.
  
  return (
    <Modal
      isOpen={show}
      onClose={onClose}
      title="rqbit server logs"
      className="max-w-7xl"
    >
      <ModalBody>
        {logsUrl ? (
          <LogStream url={logsUrl} />
        ) : (
            // If no URL but in Tauri, render Tauri-specific stream
           (window as any).__TAURI_INTERNALS__ ? (
               <LogStream url="tauri://events" /> 
           ) : (
          <ErrorComponent
            error={{ text: "HTTP API not available to stream logs" }}
          ></ErrorComponent>
           )
        )}
      </ModalBody>
      <ModalFooter>
        <Button variant="primary" onClick={onClose}>
          Close
        </Button>
      </ModalFooter>
    </Modal>
  );
};
