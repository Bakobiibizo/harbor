import toast from 'react-hot-toast';
import { useSettingsStore } from '../../stores';
import type { ThemeMode } from '../../stores/settings';
import { SunIcon, MoonIcon, MonitorIcon, SectionHeader, SettingsCard } from './shared';

export function AppearanceSection() {
  const { theme, setTheme } = useSettingsStore();

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
    </div>
  );
}
