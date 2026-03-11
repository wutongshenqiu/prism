interface TopListItem {
  label: string;
  value: string | number;
  secondary?: string;
}

interface TopListProps {
  items: TopListItem[];
  emptyText?: string;
}

export default function TopList({ items, emptyText = 'No data' }: TopListProps) {
  if (items.length === 0) {
    return <div className="top-list-empty">{emptyText}</div>;
  }

  return (
    <div className="top-list">
      {items.map((item, i) => (
        <div key={item.label} className="top-list-item">
          <span className="top-list-rank">#{i + 1}</span>
          <span className="top-list-label">{item.label}</span>
          <span className="top-list-value">{item.value}</span>
          {item.secondary && (
            <span className="top-list-secondary">{item.secondary}</span>
          )}
        </div>
      ))}
    </div>
  );
}
