import { useState } from "react";
import { pickFolders, type PcScanOptions } from "../api";
import Icon from "../Icon";
import { isWindows } from "../platform";

interface Props {
  disabled?: boolean;
  /** Runs only when the person presses Start, with the folders and checks they chose. */
  onStart: (folders: string[], opts: PcScanOptions) => void;
}

/**
 * Choosing what to scan on this computer. Nothing is read until the person presses Start, and the
 * picture checks stay off unless they tick them for this scan.
 */
export default function FolderScan({ disabled, onStart }: Props) {
  const [folders, setFolders] = useState<string[]>([]);
  const [imageText, setImageText] = useState(false);
  const [photoPlaces, setPhotoPlaces] = useState(false);

  const choose = async () => {
    const picked = await pickFolders();
    if (picked.length) setFolders((f) => [...new Set([...f, ...picked])]);
  };

  if (folders.length === 0) {
    return (
      <div className="drop-actions">
        <button disabled={disabled} onClick={choose}>Choose folders</button>
      </div>
    );
  }

  return (
    <div className="folder-scan">
      <ul className="folder-list">
        {folders.map((f) => (
          <li key={f}>
            <Icon name="folder" size={14} />
            <span className="path" title={f}>{f}</span>
            <button className="quiet small" aria-label={`Don't scan ${f}`} onClick={() => setFolders((x) => x.filter((y) => y !== f))}>Remove</button>
          </li>
        ))}
      </ul>
      <button className="link small" onClick={choose}>Add another folder</button>
      <fieldset className="scan-checks">
        <legend className="small muted">Always checked: file names, text files and Word documents. Also:</legend>
        {isWindows() && <label className="check">
          <input type="checkbox" checked={imageText} onChange={(e) => setImageText(e.target.checked)} />
          <span>
            Read text in pictures
            <small className="muted">Screenshots and scanned letters or documents. Slower. Uses Windows' own text recognition, on this computer.</small>
          </span>
        </label>}
        <label className="check">
          <input type="checkbox" checked={photoPlaces} onChange={(e) => setPhotoPlaces(e.target.checked)} />
          <span>
            Check where photos were taken
            <small className="muted">Phones save the exact spot in each photo. Finds the ones that give away places like your home.</small>
          </span>
        </label>
      </fieldset>
      <div className="toolbar">
        <button className="primary" disabled={disabled} onClick={() => onStart(folders, { image_text: imageText, photo_places: photoPlaces })}>
          Start scan
        </button>
        <button className="quiet" disabled={disabled} onClick={() => setFolders([])}>Cancel</button>
      </div>
      <p className="small faint">
        Nothing is read until you press Start, and only these folders. Trace keeps what it finds, never copies of your files.
      </p>
    </div>
  );
}
