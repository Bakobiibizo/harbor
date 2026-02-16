import { useEffect, useRef } from 'react';
import { useSettingsStore } from '../../stores';
import type { ThemeMode, AccentColor, FontSize } from '../../stores/settings';
import { ACCENT_COLORS } from '../../stores/settings';

interface CustomizationPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

// Icon components for the theme section
function SunIcon(props: React.SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} {...props}>
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12 18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z"
      />
    </svg>
  );
}

function MoonIcon(props: React.SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} {...props}>
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75 21a9.753 9.753 0 009.002-5.998z"
      />
    </svg>
  );
}

function MonitorIcon(props: React.SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5} {...props}>
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25A2.25 2.25 0 015.25 3h13.5A2.25 2.25 0 0121 5.25z"
      />
    </svg>
  );
}

// Check icon for selected state
function CheckIcon(props: React.SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.5} {...props}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
    </svg>
  );
}

const THEME_OPTIONS: {
  value: ThemeMode;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}[] = [
  { value: 'dark', label: 'Dark', Icon: MoonIcon },
  { value: 'light', label: 'Light', Icon: SunIcon },
  { value: 'system', label: 'System', Icon: MonitorIcon },
];

const FONT_SIZE_OPTIONS: { value: FontSize; label: string; previewSize: string }[] = [
  { value: 'small', label: 'Small', previewSize: '13px' },
  { value: 'medium', label: 'Medium', previewSize: '15px' },
  { value: 'large', label: 'Large', previewSize: '17px' },
];

const ACCENT_COLOR_KEYS: AccentColor[] = [
  'blue',
  'purple',
  'green',
  'orange',
  'pink',
  'red',
  'teal',
  'amber',
];

export function CustomizationPanel({ isOpen, onClose }: CustomizationPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const { theme, setTheme, accentColor, setAccentColor, fontSize, setFontSize } =
    useSettingsStore();

  // Close on click outside
  useEffect(() => {
    if (!isOpen) return;

    function handleClickOutside(event: MouseEvent) {
      if (panelRef.current && !panelRef.current.contains(event.target as Node)) {
        onClose();
      }
    }

    function handleEscape(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        onClose();
      }
    }

    // Delay adding the listener to avoid the opening click immediately closing it
    const timeoutId = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);
    document.addEventListener('keydown', handleEscape);

    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      ref={panelRef}
      className="absolute left-0 top-full mt-2 w-72 rounded-xl overflow-hidden animate-fade-in-scale"
      style={{
        background: 'hsl(var(--harbor-bg-elevated))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
        boxShadow: '0 20px 40px -8px rgba(0, 0, 0, 0.35), 0 8px 16px -4px rgba(0, 0, 0, 0.2)',
        zIndex: 'var(--z-popover)',
      }}
    >
      {/* Header */}
      <div
        className="px-4 py-3 border-b"
        style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
      >
        <h3 className="text-sm font-semibold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Customize Harbor
        </h3>
        <p className="text-xs mt-0.5" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Personalize your experience
        </p>
      </div>

      <div className="p-4 space-y-5">
        {/* Theme Section */}
        <div>
          <label
            className="text-xs font-semibold uppercase tracking-wider mb-2.5 block"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            Theme
          </label>
          <div className="grid grid-cols-3 gap-2">
            {THEME_OPTIONS.map(({ value, label, Icon }) => {
              const isActive = theme === value;
              return (
                <button
                  key={value}
                  onClick={() => setTheme(value)}
                  className="flex flex-col items-center gap-1.5 px-2 py-2.5 rounded-lg transition-all duration-200"
                  style={{
                    background: isActive
                      ? 'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))'
                      : 'hsl(var(--harbor-surface-1))',
                    border: isActive
                      ? '1.5px solid hsl(var(--harbor-primary) / 0.5)'
                      : '1.5px solid transparent',
                  }}
                  title={`Switch to ${label} theme`}
                >
                  <Icon
                    className="w-4.5 h-4.5"
                    style={{
                      width: '18px',
                      height: '18px',
                      color: isActive
                        ? 'hsl(var(--harbor-primary))'
                        : 'hsl(var(--harbor-text-secondary))',
                    }}
                  />
                  <span
                    className="text-xs font-medium"
                    style={{
                      color: isActive
                        ? 'hsl(var(--harbor-primary))'
                        : 'hsl(var(--harbor-text-primary))',
                    }}
                  >
                    {label}
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Accent Color Section */}
        <div>
          <label
            className="text-xs font-semibold uppercase tracking-wider mb-2.5 block"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            Accent Color
          </label>
          <div className="grid grid-cols-4 gap-2">
            {ACCENT_COLOR_KEYS.map((colorKey) => {
              const isActive = accentColor === colorKey;
              const colorDef = ACCENT_COLORS[colorKey];
              return (
                <button
                  key={colorKey}
                  onClick={() => setAccentColor(colorKey)}
                  className="group flex flex-col items-center gap-1.5 py-2 rounded-lg transition-all duration-200"
                  style={{
                    background: isActive ? `${colorDef.swatch}15` : 'transparent',
                  }}
                  title={`Use ${colorDef.label} accent`}
                >
                  <div
                    className="relative w-7 h-7 rounded-full transition-transform duration-200 group-hover:scale-110"
                    style={{
                      background: colorDef.swatch,
                      boxShadow: isActive
                        ? `0 0 0 2.5px hsl(var(--harbor-bg-elevated)), 0 0 0 4px ${colorDef.swatch}`
                        : 'none',
                    }}
                  >
                    {isActive && (
                      <CheckIcon
                        className="absolute inset-0 m-auto w-3.5 h-3.5"
                        style={{ color: 'white' }}
                      />
                    )}
                  </div>
                  <span
                    className="text-[10px] font-medium"
                    style={{
                      color: isActive
                        ? 'hsl(var(--harbor-text-primary))'
                        : 'hsl(var(--harbor-text-tertiary))',
                    }}
                  >
                    {colorDef.label}
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Font Size Section */}
        <div>
          <label
            className="text-xs font-semibold uppercase tracking-wider mb-2.5 block"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            Font Size
          </label>
          <div className="grid grid-cols-3 gap-2">
            {FONT_SIZE_OPTIONS.map(({ value, label, previewSize }) => {
              const isActive = fontSize === value;
              return (
                <button
                  key={value}
                  onClick={() => setFontSize(value)}
                  className="flex flex-col items-center gap-1 px-2 py-2.5 rounded-lg transition-all duration-200"
                  style={{
                    background: isActive
                      ? 'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))'
                      : 'hsl(var(--harbor-surface-1))',
                    border: isActive
                      ? '1.5px solid hsl(var(--harbor-primary) / 0.5)'
                      : '1.5px solid transparent',
                  }}
                  title={`${label} font size`}
                >
                  <span
                    className="font-semibold leading-none transition-colors duration-200"
                    style={{
                      fontSize: previewSize,
                      color: isActive
                        ? 'hsl(var(--harbor-primary))'
                        : 'hsl(var(--harbor-text-secondary))',
                    }}
                  >
                    Aa
                  </span>
                  <span
                    className="text-xs font-medium mt-0.5"
                    style={{
                      color: isActive
                        ? 'hsl(var(--harbor-primary))'
                        : 'hsl(var(--harbor-text-primary))',
                    }}
                  >
                    {label}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* Footer hint */}
      <div
        className="px-4 py-2.5 border-t"
        style={{
          borderColor: 'hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-surface-1) / 0.5)',
        }}
      >
        <p
          className="text-[10px] text-center"
          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
        >
          Changes are saved automatically
        </p>
      </div>
    </div>
  );
}
