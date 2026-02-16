import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useSettingsStore } from '../../stores';
import { SectionHeader, SettingsCard, Toggle } from './shared';
import { playMessageSound } from '../../services/audioNotifications';

export function NotificationsSection() {
  const { soundEnabled, setSoundEnabled } = useSettingsStore();

  const handleToggleSound = useCallback(
    (value: boolean) => {
      setSoundEnabled(value);
      if (value) {
        playMessageSound();
        toast.success('Sound notifications enabled');
      } else {
        toast.success('Sound notifications disabled');
      }
    },
    [setSoundEnabled],
  );

  return (
    <div className="space-y-6">
      <SectionHeader
        title="Notifications"
        description="Control sound alerts for incoming events"
      />

      <SettingsCard>
        <div className="flex items-center justify-between">
          <div>
            <h4 className="font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Sound notifications
            </h4>
            <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Play a sound when you receive messages, wall posts, or community board updates
            </p>
          </div>
          <Toggle enabled={soundEnabled} onChange={handleToggleSound} />
        </div>
      </SettingsCard>

      <SettingsCard>
        <h4 className="font-medium mb-4" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Smart muting
        </h4>
        <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Sounds are automatically suppressed when you are actively viewing the relevant
          conversation, feed, or board. This prevents unnecessary alerts when you can already see
          new content arriving in real time.
        </p>
        <div className="mt-4 space-y-3">
          <div className="flex items-start gap-3">
            <div
              className="w-2 h-2 rounded-full mt-1.5 flex-shrink-0"
              style={{ background: 'hsl(var(--harbor-primary))' }}
            />
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              <span style={{ color: 'hsl(var(--harbor-text-primary))' }}>Messages</span> — muted
              when viewing the sender's conversation
            </p>
          </div>
          <div className="flex items-start gap-3">
            <div
              className="w-2 h-2 rounded-full mt-1.5 flex-shrink-0"
              style={{ background: 'hsl(var(--harbor-accent))' }}
            />
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              <span style={{ color: 'hsl(var(--harbor-text-primary))' }}>Wall posts</span> — muted
              when viewing the feed
            </p>
          </div>
          <div className="flex items-start gap-3">
            <div
              className="w-2 h-2 rounded-full mt-1.5 flex-shrink-0"
              style={{ background: 'hsl(var(--harbor-success))' }}
            />
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              <span style={{ color: 'hsl(var(--harbor-text-primary))' }}>Board posts</span> —
              muted when viewing that board
            </p>
          </div>
        </div>
      </SettingsCard>
    </div>
  );
}
