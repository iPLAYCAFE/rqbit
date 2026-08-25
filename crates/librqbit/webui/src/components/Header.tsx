import { FileInput } from "./buttons/FileInput";
import { MagnetInput } from "./buttons/MagnetInput";
import { CreateTorrentButton } from "./buttons/CreateTorrentButton";

// @ts-ignore
import Logo from "../../assets/logo.svg?react";

export const Header = ({
  title,
  version,
  settingsSlot,
}: {
  title: string;
  version: string;
  settingsSlot?: React.ReactNode;
}) => {
  return (
    <header className="bg-surface-raised border-b border-divider flex flex-nowrap h-16 shrink-0 justify-between items-center px-2 overflow-hidden">
      <div className="flex flex-nowrap items-center shrink-0 m-2">
        <Logo className="w-10 h-10 p-1" alt="logo" />
        <h1 className="flex items-center ml-2">
          <div className="text-xl lg:text-3xl font-bold truncate">{title}</div>
          <div className="bg-primary/10 text-primary text-sm lg:text-xl font-semibold px-2.5 py-0.5 rounded ml-2">
            v{version}
          </div>
        </h1>
      </div>
      <div className="flex flex-nowrap items-center gap-2">
        <MagnetInput className="justify-center" />
        <FileInput className="justify-center" />
        <CreateTorrentButton className="justify-center" />
        {settingsSlot && (
          <>
            <div className="hidden lg:block w-px h-6 bg-divider mx-2" />
            {settingsSlot}
          </>
        )}
      </div>
    </header>
  );
};
