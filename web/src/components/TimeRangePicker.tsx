import type { TimeRange } from '../types';

const RANGES: { value: TimeRange; label: string }[] = [
  { value: '5m', label: '5m' },
  { value: '15m', label: '15m' },
  { value: '1h', label: '1h' },
  { value: '6h', label: '6h' },
  { value: '24h', label: '24h' },
];

interface TimeRangePickerProps {
  value: TimeRange;
  onChange: (range: TimeRange) => void;
}

export default function TimeRangePicker({ value, onChange }: TimeRangePickerProps) {
  return (
    <div className="time-range-picker">
      {RANGES.map((r) => (
        <button
          key={r.value}
          className={`time-range-btn ${value === r.value ? 'time-range-btn--active' : ''}`}
          onClick={() => onChange(r.value)}
        >
          {r.label}
        </button>
      ))}
    </div>
  );
}
