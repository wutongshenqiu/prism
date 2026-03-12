interface TagListProps {
  items: string[];
  maxVisible?: number;
  emptyLabel?: string;
}

export default function TagList({ items, maxVisible = 3, emptyLabel = 'All' }: TagListProps) {
  if (items.length === 0) {
    return <span className="text-muted">{emptyLabel}</span>;
  }

  return (
    <div className="tag-list">
      {items.slice(0, maxVisible).map((item) => (
        <span key={item} className="tag">{item}</span>
      ))}
      {items.length > maxVisible && (
        <span
          className="tag tag-more"
          title={items.slice(maxVisible).join(', ')}
        >
          +{items.length - maxVisible}
        </span>
      )}
    </div>
  );
}
