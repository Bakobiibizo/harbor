import { useEffect, useState } from 'react';
import { useSettingsStore } from '../../stores';
import {
  grantProviderSessionConsent,
  hasProviderSessionConsent,
  type ProviderEmbedDescriptor,
} from '../../utils/providerEmbeds';

type EmbedState = 'consent' | 'loading' | 'ready' | 'unavailable';

export function ProviderEmbed({ embed }: { embed: ProviderEmbedDescriptor }) {
  const consentPersistence = useSettingsStore((state) => state.providerEmbedConsent);
  const remembered = consentPersistence === 'session' && hasProviderSessionConsent(embed.provider);
  const [state, setState] = useState<EmbedState>(
    remembered ? (navigator.onLine === false ? 'unavailable' : 'loading') : 'consent',
  );

  useEffect(() => {
    const nextRemembered =
      consentPersistence === 'session' && hasProviderSessionConsent(embed.provider);
    setState(nextRemembered ? (navigator.onLine === false ? 'unavailable' : 'loading') : 'consent');
  }, [consentPersistence, embed.embedUrl, embed.provider]);

  useEffect(() => {
    if (state !== 'loading') return;
    const timeout = window.setTimeout(() => setState('unavailable'), 12_000);
    return () => window.clearTimeout(timeout);
  }, [state]);

  const load = () => {
    if (typeof navigator !== 'undefined' && navigator.onLine === false) {
      setState('unavailable');
      return;
    }
    if (consentPersistence === 'session') {
      grantProviderSessionConsent(embed.provider);
    }
    setState('loading');
  };

  if (state === 'consent') {
    return (
      <section
        className="mt-2 rounded-lg p-4 motion-reduce:transition-none"
        style={{
          border: '1px solid hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-bg-elevated))',
          color: 'hsl(var(--harbor-text-primary))',
        }}
        aria-label={`${embed.providerLabel} player consent`}
        data-provider={embed.provider}
        data-state="consent"
      >
        <p className="text-sm font-semibold">Load the {embed.providerLabel} player?</p>
        <p className="text-xs mt-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          The safe preview above does not contact {embed.providerLabel}. Loading its player shares
          your IP address and browser details with that provider, and the provider may use cookies
          or observe playback.
        </p>
        <button
          type="button"
          className="mt-3 px-4 py-2 rounded-lg text-sm font-medium transition-colors motion-reduce:transition-none"
          style={{
            background: 'hsl(var(--harbor-primary))',
            color: 'white',
          }}
          onClick={load}
        >
          Load {embed.providerLabel} player
        </button>
        <p className="text-xs mt-2" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          {consentPersistence === 'session'
            ? `Your Privacy setting remembers this choice for ${embed.providerLabel} until Harbor closes.`
            : 'Your Privacy setting requires consent every time a player is opened.'}
        </p>
      </section>
    );
  }

  if (state === 'unavailable') {
    return (
      <section
        className="mt-2 rounded-lg p-4"
        style={{
          border: '1px solid hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-bg-elevated))',
        }}
        role="status"
        data-provider={embed.provider}
        data-state="unavailable"
      >
        <p className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          The {embed.providerLabel} player is unavailable.
        </p>
        <p className="text-xs mt-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          You may be offline or the provider may have blocked embedded playback. The safe link card
          is still available above.
        </p>
        <button
          type="button"
          className="mt-3 px-3 py-2 rounded-lg text-sm motion-reduce:transition-none"
          style={{
            border: '1px solid hsl(var(--harbor-border-subtle))',
            color: 'hsl(var(--harbor-text-primary))',
          }}
          onClick={load}
        >
          Retry player
        </button>
      </section>
    );
  }

  return (
    <section
      className="mt-2 rounded-lg overflow-hidden"
      style={{
        border: '1px solid hsl(var(--harbor-border-subtle))',
        background: 'hsl(var(--harbor-surface-1))',
      }}
      aria-label={`${embed.providerLabel} embedded player`}
      data-provider={embed.provider}
      data-state={state}
    >
      <div
        className="relative w-full mx-auto"
        style={{
          aspectRatio: embed.aspectRatio,
          minHeight: embed.minimumHeight,
          maxWidth: embed.maximumWidth,
        }}
      >
        <iframe
          src={embed.embedUrl}
          title={embed.title}
          className="absolute inset-0 h-full w-full border-0"
          sandbox="allow-scripts allow-same-origin allow-presentation"
          allow="autoplay; encrypted-media; fullscreen; picture-in-picture"
          referrerPolicy="no-referrer"
          loading="lazy"
          allowFullScreen
          onLoad={() => setState('ready')}
          onError={() => setState('unavailable')}
        />
        {state === 'loading' && (
          <div
            className="absolute inset-0 grid place-items-center pointer-events-none"
            style={{ background: 'hsl(var(--harbor-surface-1) / 0.92)' }}
            role="status"
          >
            <span style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Loading {embed.providerLabel} player…
            </span>
          </div>
        )}
      </div>
      <div className="flex justify-end p-2">
        <button
          type="button"
          className="px-3 py-1.5 rounded text-xs motion-reduce:transition-none"
          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          onClick={() => setState('consent')}
        >
          Unload this player
        </button>
      </div>
    </section>
  );
}
