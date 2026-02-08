import { useState } from 'react';
import { UserIcon, LockIcon, ShieldIcon, ChevronRightIcon } from '../components/icons';
import { PaletteIcon, DownloadIcon } from './settings/shared';
import {
  ProfileSection,
  AppearanceSection,
  SecuritySection,
  PrivacySection,
  UpdatesSection,
} from './settings/index';

const sections = [
  { id: 'profile', label: 'Profile', icon: UserIcon, description: 'Your identity and bio' },
  { id: 'appearance', label: 'Appearance', icon: PaletteIcon, description: 'Theme and display' },
  { id: 'security', label: 'Security', icon: LockIcon, description: 'Passphrase and keys' },
  { id: 'privacy', label: 'Privacy', icon: ShieldIcon, description: 'Visibility controls' },
  { id: 'updates', label: 'Updates', icon: DownloadIcon, description: 'Check for new versions' },
] as const;

const sectionComponents: Record<string, React.FC> = {
  profile: ProfileSection,
  appearance: AppearanceSection,
  security: SecuritySection,
  privacy: PrivacySection,
  updates: UpdatesSection,
};

export function SettingsPage() {
  const [activeSection, setActiveSection] = useState<string>('profile');

  const ActiveComponent = sectionComponents[activeSection];

  return (
    <div className="h-full flex" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Settings sidebar */}
      <div
        className="w-1/3 max-w-xs flex flex-col border-r flex-shrink-0"
        style={{
          borderColor: 'hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-bg-elevated))',
        }}
      >
        <div className="p-4 border-b" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
          <h2 className="text-lg font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
            Settings
          </h2>
          <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Customize your experience
          </p>
        </div>

        <nav className="flex-1 p-2 space-y-1">
          {sections.map((section) => {
            const Icon = section.icon;
            const isActive = activeSection === section.id;

            return (
              <button
                key={section.id}
                onClick={() => setActiveSection(section.id)}
                className="w-full flex items-center gap-3 p-3 rounded-lg text-left transition-all duration-200"
                style={{
                  background: isActive
                    ? 'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))'
                    : 'transparent',
                  border: isActive
                    ? '1px solid hsl(var(--harbor-primary) / 0.2)'
                    : '1px solid transparent',
                }}
              >
                <div
                  className="w-9 h-9 rounded-lg flex items-center justify-center"
                  style={{
                    background: isActive
                      ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                      : 'hsl(var(--harbor-surface-2))',
                  }}
                >
                  <Icon
                    className="w-5 h-5"
                    style={{
                      color: isActive ? 'white' : 'hsl(var(--harbor-text-secondary))',
                    }}
                  />
                </div>
                <div className="flex-1 min-w-0">
                  <p
                    className="font-medium text-sm"
                    style={{
                      color: isActive
                        ? 'hsl(var(--harbor-primary))'
                        : 'hsl(var(--harbor-text-primary))',
                    }}
                  >
                    {section.label}
                  </p>
                  <p
                    className="text-xs truncate"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    {section.description}
                  </p>
                </div>
                <ChevronRightIcon
                  className="w-4 h-4"
                  style={{
                    color: isActive
                      ? 'hsl(var(--harbor-primary))'
                      : 'hsl(var(--harbor-text-tertiary))',
                  }}
                />
              </button>
            );
          })}
        </nav>

        <div className="p-4 border-t" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
          <p className="text-xs text-center" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            Harbor v1.0.0
          </p>
        </div>
      </div>

      {/* Settings content */}
      <div className="flex-1 overflow-y-auto p-8">
        <div className="max-w-2xl">{ActiveComponent && <ActiveComponent />}</div>
      </div>
    </div>
  );
}
