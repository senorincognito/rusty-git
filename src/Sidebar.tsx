import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import {
  addOriginRemote,
  checkoutLocalBranch,
  getLocalBranches,
  getOrigin,
  type BranchInfo,
  type RemoteInfo,
} from "./git";

function Section({
  title,
  count,
  children,
}: {
  title: string;
  count?: number;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <section className="sidebox">
      <button className="sidebox-head" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <span className="chev">{open ? "▾" : "▸"}</span>
        <span>{title}</span>
        {count !== undefined && <span className="count">{count}</span>}
      </button>
      {open && <div className="sidebox-body">{children}</div>}
    </section>
  );
}

function LocalBranches({
  path,
  refreshKey,
  onChanged,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
}) {
  const [branches, setBranches] = useState<BranchInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(0);

  const refresh = useCallback(async () => {
    const id = ++latest.current;
    try {
      const b = await getLocalBranches(path);
      if (id !== latest.current) return;
      setBranches(b);
      setError(null);
    } catch (e) {
      if (id === latest.current) setError(String(e));
    }
  }, [path]);

  const [switching, setSwitching] = useState(false);
  const switchTo = async (b: BranchInfo) => {
    if (b.isHead || switching) return;
    setSwitching(true);
    try {
      await checkoutLocalBranch(path, b.name);
      setError(null);
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setSwitching(false);
    }
  };

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  return (
    <Section title="Local branches" count={branches?.length}>
      {error && <p className="error side-msg">{error}</p>}
      {branches?.length === 0 && <p className="muted side-msg">No branches yet.</p>}
      <ul className="branchlist">
        {branches?.map((b) => (
          <li
            key={b.name}
            className={b.isHead ? "current" : ""}
            title={b.isHead ? `${b.name} (current)` : `Double-click to check out ${b.name}`}
            onDoubleClick={() => switchTo(b)}
          >
            <span className="bname">{b.name}</span>
            {b.behind > 0 && <span className="sync">↓{b.behind}</span>}
            {b.ahead > 0 && <span className="sync">↑{b.ahead}</span>}
          </li>
        ))}
      </ul>
    </Section>
  );
}

function AddOrigin({ path, onAdded }: { path: string; onAdded: () => void }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      await addOriginRemote(path, url);
      setError(null);
      setUrl("");
      onAdded();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="addremote"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <p className="muted">No origin yet. Add the URL of the remote repository.</p>
      <input
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="https://github.com/user/repo.git"
        spellCheck={false}
        autoComplete="off"
      />
      {error && <p className="error">{error}</p>}
      <button className="primary" type="submit" disabled={busy || url.trim() === ""}>
        Add origin
      </button>
    </form>
  );
}

function Remotes({
  path,
  refreshKey,
  onChanged,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
}) {
  const [origin, setOrigin] = useState<RemoteInfo | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(0);

  const refresh = useCallback(async () => {
    const id = ++latest.current;
    try {
      const o = await getOrigin(path);
      if (id !== latest.current) return;
      setOrigin(o);
      setError(null);
    } catch (e) {
      if (id === latest.current) setError(String(e));
    }
  }, [path]);

  useEffect(() => {
    setOrigin(undefined);
  }, [path]);

  useEffect(() => {
    refresh();
  }, [refresh, refreshKey]);

  return (
    <Section title="Remotes" count={origin ? origin.branches.length : undefined}>
      {error && <p className="error side-msg">{error}</p>}
      {origin === null && <AddOrigin path={path} onAdded={onChanged} />}
      {origin && (
        <>
          <div className="remote-head" title={origin.url}>
            <span className="rname">{origin.name}</span>
            <span className="rurl">{origin.url}</span>
          </div>
          {origin.branches.length === 0 && (
            <p className="muted side-msg">No remote branches yet. Fetch to load them.</p>
          )}
          <ul className="branchlist">
            {origin.branches.map((b) => (
              <li key={b} title={`${origin.name}/${b}`}>
                <span className="bname">{b}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </Section>
  );
}

export default function Sidebar({
  path,
  refreshKey,
  onChanged,
}: {
  path: string;
  refreshKey: number;
  onChanged: () => void;
}) {
  return (
    <nav className="sidebar">
      <LocalBranches path={path} refreshKey={refreshKey} onChanged={onChanged} />
      <Remotes path={path} refreshKey={refreshKey} onChanged={onChanged} />
    </nav>
  );
}
