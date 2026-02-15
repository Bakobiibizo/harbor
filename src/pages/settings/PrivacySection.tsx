import toast from 'react-hot-toast';
import { useSettingsStore } from '../../stores';
import { SectionHeader, SettingsCard, Toggle } from './shared';

export function PrivacySection() {
  const {
    showReadReceipts,
    showOnlineStatus,
    defaultVisibility,
    setShowReadReceipts,
    setShowOnlineStatus,
    setDefaultVisibility,
  } = useSettingsStore();

  const handleOnlineStatusChange = (value: boolean) => {
    setShowOnlineStatus(value);
    toast.success(value ? 'Online status visible to contacts' : 'Online status hidden');
  };

  return (
    <div className="space-y-6">
      <SectionHeader title="Privacy" description="Control who can see your content" />

      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Default post visibility
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Who can see your new posts by default
        </p>
        <select
          value={defaultVisibility}
          onChange={(e) => setDefaultVisibility(e.target.value as 'contacts' | 'public')}
          className="w-full px-4 py-3 rounded-lg text-sm"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
            color: 'hsl(var(--harbor-text-primary))',
          }}
        >
          <option value="contacts">Contacts only</option>
          <option value="public">Anyone with the link</option>
        </select>
      </SettingsCard>

      <SettingsCard>
        <div className="flex items-center justify-between">
          <div>
            <h4 className="font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Message read notifications
            </h4>
            <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Let others know when you've read their messages
            </p>
          </div>
          <Toggle enabled={showReadReceipts} onChange={setShowReadReceipts} />
        </div>
      </SettingsCard>

      <SettingsCard>
        <div className="flex items-center justify-between">
          <div>
            <h4 className="font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Show online status
            </h4>
            <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Show when you're online to your contacts
            </p>
          </div>
          <Toggle enabled={showOnlineStatus} onChange={handleOnlineStatusChange} />
        </div>
      </SettingsCard>
    </div>
  );
}
