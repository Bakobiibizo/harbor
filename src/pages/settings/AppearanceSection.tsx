import toast from 'react-hot-toast';
import { useSettingsStore } from '../../stores';
import { ACCENT_COLORS } from '../../stores/settings';
import type { AccentColor, FontSize, ThemeMode } from '../../stores/settings';
import { SunIcon, MoonIcon, MonitorIcon, SectionHeader, SettingsCard } from './shared';

export function AppearanceSection() {
  const { theme, setTheme, accentColor, setAccentColor, fontSize, setFontSize } =
    useSettingsStore();

  const accentOptions = Object.entries(ACCENT_COLORS) as [
    AccentColor,
    (typeof ACCENT_COLORS)[AccentColor],
  ][];
  const fontOptions: { value: FontSize; label: string; sample: string }[] = [
    { value: 'small', label: 'Small', sample: '13px' },
    { value: 'medium', label: 'Medium', sample: '15px' },
    { value: 'large', label: 'Large', sample: '17px' },
  ];

  return (
    <div className="space-y-6">
      <SectionHeader title="Appearance" description="Customize how Harbor looks" />

      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Theme
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Choose your preferred color scheme
        </p>

        <div className="grid grid-cols-3 gap-3">
          {[
            { value: 'system' as ThemeMode, label: 'System', Icon: MonitorIcon },
            { value: 'light' as ThemeMode, label: 'Light', Icon: SunIcon },
            { value: 'dark' as ThemeMode, label: 'Dark', Icon: MoonIcon },
          ].map(({ value, label, Icon }) => {
            const isActive = theme === value;
            return (
              <button
                key={value}
                onClick={() => {
                  setTheme(value);
                  toast.success(`Theme set to ${label.toLowerCase()}`);
                }}
                className="flex flex-col items-center gap-2 p-4 rounded-lg transition-all duration-200"
                style={{
                  background: isActive
                    ? 'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))'
                    : 'hsl(var(--harbor-surface-1))',
                  border: isActive
                    ? '2px solid hsl(var(--harbor-primary))'
                    : '2px solid transparent',
                }}
              >
                <Icon
                  className="w-6 h-6"
                  style={{
                    color: isActive
                      ? 'hsl(var(--harbor-primary))'
                      : 'hsl(var(--harbor-text-secondary))',
                  }}
                />
                <span
                  className="text-sm font-medium"
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

        <p className="text-xs mt-3" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          System follows your operating system's theme preference
        </p>
      </SettingsCard>

      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Color preset
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Harbor is the branded default. Accent presets change interactive color without changing
          the product icon or identity.
        </p>
        <div className="grid grid-cols-3 sm:grid-cols-5 gap-3">
          {accentOptions.map(([value, definition]) => {
            const isActive = accentColor === value;
            return (
              <button
                key={value}
                type="button"
                aria-pressed={isActive}
                onClick={() => {
                  setAccentColor(value);
                  toast.success(`Color preset set to ${definition.label}`);
                }}
                className="flex flex-col items-center gap-2 p-3 rounded-lg transition-all duration-200"
                style={{
                  color: 'hsl(var(--harbor-text-primary))',
                  background: isActive
                    ? 'hsl(var(--harbor-primary) / 0.12)'
                    : 'hsl(var(--harbor-surface-1))',
                  border: isActive
                    ? '2px solid hsl(var(--harbor-primary))'
                    : '2px solid transparent',
                }}
              >
                <span
                  aria-hidden="true"
                  className="w-7 h-7 rounded-full"
                  style={{
                    background: `linear-gradient(135deg, ${definition.swatch}, hsl(${definition.accent}))`,
                  }}
                />
                <span className="text-xs font-medium">{definition.label}</span>
              </button>
            );
          })}
        </div>
      </SettingsCard>

      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Text size
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Adjust interface text while preserving layout and operating-system scaling.
        </p>
        <div className="grid grid-cols-3 gap-3">
          {fontOptions.map(({ value, label, sample }) => {
            const isActive = fontSize === value;
            return (
              <button
                key={value}
                type="button"
                aria-pressed={isActive}
                onClick={() => {
                  setFontSize(value);
                  toast.success(`Text size set to ${label.toLowerCase()}`);
                }}
                className="p-4 rounded-lg transition-all duration-200"
                style={{
                  color: 'hsl(var(--harbor-text-primary))',
                  background: isActive
                    ? 'hsl(var(--harbor-primary) / 0.12)'
                    : 'hsl(var(--harbor-surface-1))',
                  border: isActive
                    ? '2px solid hsl(var(--harbor-primary))'
                    : '2px solid transparent',
                  fontSize: sample,
                }}
              >
                {label}
              </button>
            );
          })}
        </div>
      </SettingsCard>
    </div>
  );
}
