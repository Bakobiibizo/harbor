import { useEffect, useRef, useState } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore, useSettingsStore, useWallStore } from '../../stores';
import type { WallContentType } from '../../stores';
import type { PostVisibility, ResolvedMention } from '../../types';
import { mentionsService } from '../../services';
import { MentionResolution } from '../identity';
import { contentTypeForPost } from '../../utils/postModality';
import { createLogger } from '../../utils/logger';
import { safeIdentityLabel } from '../../utils/relayName';

const log = createLogger('ComposePostModal');
type MediaType = 'image' | 'video' | 'audio';
type PendingMedia = { type: MediaType; url: string; name: string; file: File };

const CONTENT_TYPES: {
  type: WallContentType;
  label: string;
  placeholder: string;
  limit?: number;
}[] = [
  { type: 'post', label: 'Post', placeholder: 'Share your thoughts, ideas, or creative work...' },
  { type: 'thought', label: 'Tweet', placeholder: "What's on your mind?", limit: 280 },
  { type: 'image', label: 'Image', placeholder: 'Add a caption for your image...' },
  { type: 'video', label: 'Video', placeholder: 'Add a caption for your video...' },
  { type: 'audio', label: 'Audio', placeholder: 'Add a caption for your audio...' },
];

export function ComposePostModal({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  const identityState = useIdentityStore((state) => state.state);
  const defaultVisibility = useSettingsStore((state) => state.defaultVisibility);
  const { createPost, loadPosts } = useWallStore();
  const [content, setContent] = useState('');
  const [contentType, setContentType] = useState<WallContentType>('post');
  const [visibility, setVisibility] = useState<PostVisibility>(defaultVisibility);
  const [media, setMedia] = useState<PendingMedia[]>([]);
  const [resolvedMentions, setResolvedMentions] = useState<ResolvedMention[]>([]);
  const [isPublishing, setIsPublishing] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const mediaTypeRef = useRef<MediaType>('image');
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  const identity = identityState.status === 'unlocked' ? identityState.identity : null;
  const config = CONTENT_TYPES.find((item) => item.type === contentType) ?? CONTENT_TYPES[0];

  useEffect(() => {
    if (!isOpen) return;
    returnFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!content && media.length === 0) setVisibility(defaultVisibility);
    window.requestAnimationFrame(() => textareaRef.current?.focus());

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        onCloseRef.current();
        return;
      }
      if (event.key !== 'Tab' || !dialogRef.current) return;
      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), textarea:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      window.requestAnimationFrame(() => returnFocusRef.current?.focus());
    };
  }, [isOpen]);

  if (!identity) return null;

  const selectMedia = (type: MediaType) => {
    mediaTypeRef.current = type;
    if (!fileInputRef.current) return;
    fileInputRef.current.accept = `${type}/*`;
    fileInputRef.current.click();
  };

  const addMedia = (file: File | undefined) => {
    if (!file) return;
    if (file.size > 10 * 1024 * 1024) {
      toast.error('File size must be less than 10MB');
      return;
    }
    const type = mediaTypeRef.current;
    setMedia((current) => [
      ...current,
      { type, url: URL.createObjectURL(file), name: file.name, file },
    ]);
    setContentType(type);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const removeMedia = (index: number) => {
    setMedia((current) => {
      URL.revokeObjectURL(current[index].url);
      return current.filter((_, mediaIndex) => mediaIndex !== index);
    });
  };

  const clearDraft = () => {
    media.forEach((item) => URL.revokeObjectURL(item.url));
    setContent('');
    setContentType('post');
    setVisibility(defaultVisibility);
    setMedia([]);
    setResolvedMentions([]);
  };

  const publish = async () => {
    if ((!content.trim() && media.length === 0) || isPublishing) return;
    if (config.limit && content.length > config.limit) return;
    if (resolvedMentions.some((mention) => mention.status === 'blocked')) {
      toast.error('Remove blocked mentions before publishing');
      return;
    }
    if (resolvedMentions.length > 0 && media.length > 0) {
      toast.error('Mentioned posts cannot include attachments yet');
      return;
    }

    setIsPublishing(true);
    const derivedContentType = contentTypeForPost(contentType, media);
    try {
      if (resolvedMentions.length > 0) {
        await mentionsService.publish({
          contentType: derivedContentType === 'post' ? 'text' : derivedContentType,
          contentText: content.trim(),
          visibility,
          mentions: resolvedMentions.map((mention) => ({
            qualifiedName: mention.qualifiedName,
            intent: 'notify',
            authorizedPeerId: mention.status === 'known' ? mention.peerId : undefined,
            claimDigest: mention.claimDigest,
          })),
        });
        await loadPosts();
      } else {
        await createPost(
          content.trim(),
          derivedContentType,
          media.length ? media : undefined,
          visibility,
        );
      }
      toast.success(`${config.label} published`);
      clearDraft();
      onClose();
    } catch (error) {
      log.error('Failed to publish post', error);
      toast.error('Failed to publish post');
    } finally {
      setIsPublishing(false);
    }
  };

  return (
    <div
      className={`${isOpen ? 'flex' : 'hidden'} fixed inset-0 z-[100] items-center justify-center p-4`}
      style={{ background: 'hsl(215 30% 4% / 0.72)' }}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="compose-post-title"
        className="max-h-[90vh] w-full max-w-2xl overflow-y-auto rounded-xl shadow-2xl"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <header
          className="flex items-center justify-between border-b px-5 py-4"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <div>
            <h2
              id="compose-post-title"
              className="text-lg font-semibold"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              Create a post
            </h2>
            <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              Posting as {safeIdentityLabel(identity)}
            </p>
          </div>
          <button
            type="button"
            aria-label="Close composer"
            onClick={onClose}
            className="harbor-interactive rounded-lg px-3 py-2"
            style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          >
            Close
          </button>
        </header>

        <div
          className="flex flex-wrap gap-2 border-b px-5 py-3"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          {CONTENT_TYPES.map((item) => (
            <button
              key={item.type}
              type="button"
              aria-pressed={contentType === item.type}
              onClick={() => setContentType(item.type)}
              className="harbor-interactive rounded-full px-3 py-1.5 text-xs font-semibold"
              style={{
                background:
                  contentType === item.type
                    ? 'hsl(var(--harbor-primary))'
                    : 'hsl(var(--harbor-surface-1))',
                color: contentType === item.type ? 'white' : 'hsl(var(--harbor-text-secondary))',
              }}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="p-5">
          <textarea
            ref={textareaRef}
            value={content}
            rows={6}
            maxLength={config.limit}
            placeholder={config.placeholder}
            onChange={(event) => setContent(event.target.value)}
            className="w-full resize-none rounded-lg p-3"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              color: 'hsl(var(--harbor-text-primary))',
              outline: 'none',
            }}
          />
          <MentionResolution text={content} onResolved={setResolvedMentions} />
          {config.limit && (
            <p
              className="mt-1 text-right text-xs"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            >
              {content.length}/{config.limit}
            </p>
          )}

          {media.length > 0 && (
            <div className="mt-4 flex flex-wrap gap-3">
              {media.map((item, index) => (
                <div
                  key={`${item.name}-${index}`}
                  className="relative overflow-hidden rounded-lg"
                  style={{ background: 'hsl(var(--harbor-surface-1))' }}
                >
                  {item.type === 'image' ? (
                    <img src={item.url} alt={item.name} className="h-28 w-32 object-cover" />
                  ) : item.type === 'video' ? (
                    <video src={item.url} className="h-28 w-32 object-cover" />
                  ) : (
                    <audio src={item.url} controls className="m-3 w-48" />
                  )}
                  <button
                    type="button"
                    aria-label={`Remove ${item.name}`}
                    onClick={() => removeMedia(index)}
                    className="absolute right-1 top-1 rounded-full px-2 py-1 text-xs text-white"
                    style={{ background: 'hsl(var(--harbor-error))' }}
                  >
                    Remove
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <footer
          className="flex flex-wrap items-center justify-between gap-3 border-t px-5 py-4"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <div className="flex flex-wrap items-center gap-2">
            {(['image', 'video', 'audio'] as const).map((type) => (
              <button
                key={type}
                type="button"
                onClick={() => selectMedia(type)}
                className="harbor-interactive rounded-lg px-3 py-2 text-sm capitalize"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-secondary))',
                }}
              >
                Add {type}
              </button>
            ))}
            <button
              type="button"
              aria-pressed={visibility === 'public'}
              onClick={() => setVisibility(visibility === 'public' ? 'contacts' : 'public')}
              title="Public posts appear in public previews and RSS. Contacts posts are shared only with contacts who have wall access."
              className="harbor-interactive rounded-lg px-3 py-2 text-sm font-semibold"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
            >
              {visibility === 'public' ? 'Public' : 'Contacts'}
            </button>
          </div>
          <div className="flex gap-2">
            {(content || media.length > 0) && (
              <button
                type="button"
                onClick={clearDraft}
                className="harbor-interactive rounded-lg px-4 py-2 text-sm"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                Discard
              </button>
            )}
            <button
              type="button"
              onClick={publish}
              disabled={isPublishing || (!content.trim() && media.length === 0)}
              className="harbor-interactive rounded-lg px-5 py-2 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:opacity-50"
              style={{ background: 'hsl(var(--harbor-primary))' }}
            >
              {isPublishing ? 'Publishing...' : 'Publish'}
            </button>
          </div>
        </footer>
        <input
          ref={fileInputRef}
          type="file"
          className="hidden"
          onChange={(event) => addMedia(event.target.files?.[0])}
        />
      </div>
    </div>
  );
}
