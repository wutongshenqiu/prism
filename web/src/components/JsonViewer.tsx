import { useState, useMemo } from 'react';
import { Copy, ChevronRight, ChevronDown } from 'lucide-react';

interface JsonViewerProps {
  data: string;
  defaultExpanded?: boolean;
}

function JsonNode({ name, value, depth, defaultExpanded }: {
  name?: string;
  value: unknown;
  depth: number;
  defaultExpanded: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded && depth < 2);

  // Leaf types: null, boolean, number, string
  const leaf = (className: string, display: string) => (
    <div className="json-line" style={{ paddingLeft: depth * 16 }}>
      {name != null && <span className="json-key">{name}: </span>}
      <span className={className}>{display}</span>
    </div>
  );

  if (value === null) return leaf('json-null', 'null');
  if (typeof value === 'boolean') return leaf('json-boolean', String(value));
  if (typeof value === 'number') return leaf('json-number', String(value));
  if (typeof value === 'string') return leaf('json-string', `"${value}"`);

  if (Array.isArray(value)) {
    if (value.length === 0) {
      return (
        <div className="json-line" style={{ paddingLeft: depth * 16 }}>
          {name != null && <span className="json-key">{name}: </span>}
          <span className="json-bracket">[]</span>
        </div>
      );
    }
    return (
      <div>
        <div
          className="json-line json-toggle"
          style={{ paddingLeft: depth * 16 }}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          {name != null && <span className="json-key">{name}: </span>}
          <span className="json-bracket">[</span>
          {!expanded && <span className="json-collapsed">{value.length} items]</span>}
        </div>
        {expanded && (
          <>
            {value.map((item, i) => (
              <JsonNode key={i} value={item} depth={depth + 1} defaultExpanded={defaultExpanded} />
            ))}
            <div className="json-line" style={{ paddingLeft: depth * 16 }}>
              <span className="json-bracket">]</span>
            </div>
          </>
        )}
      </div>
    );
  }

  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length === 0) {
      return (
        <div className="json-line" style={{ paddingLeft: depth * 16 }}>
          {name != null && <span className="json-key">{name}: </span>}
          <span className="json-bracket">{'{}'}</span>
        </div>
      );
    }
    return (
      <div>
        <div
          className="json-line json-toggle"
          style={{ paddingLeft: depth * 16 }}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          {name != null && <span className="json-key">{name}: </span>}
          <span className="json-bracket">{'{'}</span>
          {!expanded && <span className="json-collapsed">{entries.length} keys{'}'}</span>}
        </div>
        {expanded && (
          <>
            {entries.map(([k, v]) => (
              <JsonNode key={k} name={k} value={v} depth={depth + 1} defaultExpanded={defaultExpanded} />
            ))}
            <div className="json-line" style={{ paddingLeft: depth * 16 }}>
              <span className="json-bracket">{'}'}</span>
            </div>
          </>
        )}
      </div>
    );
  }

  return (
    <div className="json-line" style={{ paddingLeft: depth * 16 }}>
      {name != null && <span className="json-key">{name}: </span>}
      <span>{String(value)}</span>
    </div>
  );
}

export default function JsonViewer({ data, defaultExpanded = true }: JsonViewerProps) {
  const parsed = useMemo(() => {
    try {
      return { ok: true as const, value: JSON.parse(data) };
    } catch {
      return { ok: false as const };
    }
  }, [data]);

  const handleCopy = () => {
    const text = parsed.ok ? JSON.stringify(parsed.value, null, 2) : data;
    navigator.clipboard.writeText(text);
  };

  return (
    <div className="json-viewer">
      <button className="json-copy-btn btn btn-ghost btn-sm" onClick={handleCopy} title="Copy">
        <Copy size={12} />
      </button>
      <div className="json-content">
        {parsed.ok ? (
          <JsonNode value={parsed.value} depth={0} defaultExpanded={defaultExpanded} />
        ) : (
          <pre className="json-raw">{data}</pre>
        )}
      </div>
    </div>
  );
}
