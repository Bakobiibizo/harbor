import type { ModalityFilter as ModalityFilterValue } from '../../utils/postModality';

const OPTIONS: { value: ModalityFilterValue; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'images', label: 'Images' },
  { value: 'videos', label: 'Video' },
  { value: 'audio', label: 'Audio' },
];

export function ModalityFilter({
  value,
  onChange,
  label = 'Filter posts by media type',
}: {
  value: ModalityFilterValue;
  onChange: (value: ModalityFilterValue) => void;
  label?: string;
}) {
  return (
    <div
      role="group"
      aria-label={label}
      className="inline-flex flex-wrap gap-1 rounded-lg p-1"
      style={{
        background: 'hsl(var(--harbor-surface-1))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      {OPTIONS.map((option) => {
        const selected = value === option.value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(option.value)}
            className="harbor-interactive rounded-md px-3 py-1.5 text-sm font-medium"
            style={{
              background: selected ? 'hsl(var(--harbor-primary))' : 'transparent',
              color: selected ? 'white' : 'hsl(var(--harbor-text-secondary))',
            }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
