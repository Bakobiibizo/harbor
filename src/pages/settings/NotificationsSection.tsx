import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useSettingsStore } from '../../stores';
import { Toggle } from './shared';
import { playMessageSound } from '../../services/audioNotifications';

export function NotificationsSection() {
  const { soundEnabled, setSoundEnabled } = useSettingsStore();

  const handleToggleSound = useCallback(
    (value: boolean) => {
      setSoundEnabled(value);
      if (value) {
        // Play a preview sound so the user hears what it sounds like
        // Temporarily ensure the setting is enabled before playing
        // (setState is synchronous in Zustand so this works)
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
      <div>
        <h3
          className="text-xl font-semibold mb-1"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Notifications
        </h3>
        <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Control sound alerts for incoming events
        </p>
      </div>

      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
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
      </div>

      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
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
              <span style={{ color: 'hsl(var(--harbor-text-primary))' }}>Messages</span> -- muted
              when viewing the sender's conversation
            </p>
          </div>
          <div className="flex items-start gap-3">
            <div
              className="w-2 h-2 rounded-full mt-1.5 flex-shrink-0"
              style={{ background: 'hsl(var(--harbor-accent))' }}
            />
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              <span style={{ color: 'hsl(var(--harbor-text-primary))' }}>Wall posts</span> -- muted
              when viewing the feed
            </p>
          </div>
          <div className="flex items-start gap-3">
            <div
              className="w-2 h-2 rounded-full mt-1.5 flex-shrink-0"
              style={{ background: 'hsl(var(--harbor-success))' }}
            />
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              <span style={{ color: 'hsl(var(--harbor-text-primary))' }}>Board posts</span> --
              muted when viewing that board
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
