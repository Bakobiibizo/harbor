import { useState, useMemo, useRef, useEffect, useCallback, KeyboardEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import toast from 'react-hot-toast';
import { createLogger } from '../utils/logger';
import {
  ChatIcon,
  SearchIcon,
  PlusIcon,
  SendIcon,
  PhoneIcon,
  EllipsisIcon,
  ChevronUpIcon,
  ChevronDownIcon,
  XIcon,
  PaperclipIcon,
  PencilIcon,
  CheckIcon,
} from '../components/icons';
import { useContactsStore, useMessagingStore } from '../stores';
import { getInitials, getContactColor, formatRelativeTime } from '../utils/formatting';
import { EmojiPicker } from '../components/common/EmojiPicker';

const log = createLogger('Chat');

// --- Media attachment types and helpers ---

interface PendingAttachment {
  file: File;
  type: 'image' | 'video';
  previewUrl: string;
  name: string;
  size: number;
}

const ACCEPTED_IMAGE_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];
const ACCEPTED_VIDEO_TYPES = ['video/mp4', 'video/webm'];
const ALL_ACCEPTED_TYPES = [...ACCEPTED_IMAGE_TYPES, ...ACCEPTED_VIDEO_TYPES];
const MAX_FILE_SIZE = 10 * 1024 * 1024; // 10MB
const MEDIA_MARKER_PATTERN = /\[media:([^\]]+):([^\]]+)\]/;

/** Format file size for display */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Parse message content into text segments and media segments */
function parseMessageContent(content: string): { type: 'text' | 'media'; value: string; mimeType?: string }[] {
  const segments: { type: 'text' | 'media'; value: string; mimeType?: string }[] = [];
  let lastIndex = 0;

  const regex = new RegExp(MEDIA_MARKER_PATTERN.source, 'g');
  let match;

  while ((match = regex.exec(content)) !== null) {
    // Add text before this marker
    if (match.index > lastIndex) {
      const text = content.slice(lastIndex, match.index).trim();
      if (text) {
        segments.push({ type: 'text', value: text });
      }
    }

    // Add the media segment
    segments.push({ type: 'media', value: match[1], mimeType: match[2] });

    lastIndex = match.index + match[0].length;
  }

  // Add remaining text after last marker
  if (lastIndex < content.length) {
    const text = content.slice(lastIndex).trim();
    if (text) {
      segments.push({ type: 'text', value: text });
    }
  }

  // If no markers found, return the full content as text
  if (segments.length === 0 && content.trim()) {
    segments.push({ type: 'text', value: content });
  }

  return segments;
}

/** Check if message content contains any media markers */
function hasMediaContent(content: string): boolean {
  return MEDIA_MARKER_PATTERN.test(content);
}

/** Inline media display component for chat messages */
function ChatMediaDisplay({ url, mimeType }: { url: string; mimeType: string; isMine?: boolean }) {
  const [fullscreen, setFullscreen] = useState(false);
  const isVideo = mimeType.startsWith('video/');

  return (
    <>
      {isVideo ? (
        <video
          src={url}
          controls
          className="rounded-lg max-w-full"
          style={{ maxHeight: '300px' }}
          preload="metadata"
        />
      ) : (
        <img
          src={url}
          alt="Shared image"
          className="rounded-lg max-w-full cursor-pointer hover:opacity-90 transition-opacity"
          style={{ maxHeight: '300px' }}
          onClick={() => setFullscreen(true)}
          loading="lazy"
        />
      )}

      {/* Fullscreen lightbox for images */}
      {fullscreen && !isVideo && (
        <div
          className="fixed inset-0 z-[200] flex items-center justify-center bg-black/80 cursor-pointer"
          onClick={() => setFullscreen(false)}
        >
          <button
            className="absolute top-4 right-4 p-2 rounded-full bg-black/50 text-white hover:bg-black/70 transition-colors"
            onClick={(e) => {
              e.stopPropagation();
              setFullscreen(false);
            }}
          >
            <XIcon className="w-6 h-6" />
          </button>
          <img
            src={url}
            alt="Full size image"
            className="max-w-[90vw] max-h-[90vh] object-contain rounded-lg"
            onClick={(e) => e.stopPropagation()}
          />
        </div>
      )}
    </>
  );
}

// Component to highlight matching text segments within a message
function HighlightedText({
  text,
  query,
  isActiveMatch,
}: {
  text: string;
  query: string;
  isActiveMatch: boolean;
}) {
  if (!query.trim()) {
    return <>{text}</>;
  }

  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const parts: { text: string; highlight: boolean }[] = [];
  let lastIndex = 0;

  let searchFrom = 0;
  while (searchFrom < lowerText.length) {
    const matchIndex = lowerText.indexOf(lowerQuery, searchFrom);
    if (matchIndex === -1) break;

    // Add non-matching text before this match
    if (matchIndex > lastIndex) {
      parts.push({ text: text.slice(lastIndex, matchIndex), highlight: false });
    }

    // Add the matching text
    parts.push({
      text: text.slice(matchIndex, matchIndex + query.length),
      highlight: true,
    });

    lastIndex = matchIndex + query.length;
    searchFrom = lastIndex;
  }

  // Add remaining text after last match
  if (lastIndex < text.length) {
    parts.push({ text: text.slice(lastIndex), highlight: false });
  }

  // If no matches found, return plain text
  if (parts.length === 0) {
    return <>{text}</>;
  }

  return (
    <>
      {parts.map((part, i) =>
        part.highlight ? (
          <mark
            key={i}
            className="rounded px-0.5"
            style={{
              background: isActiveMatch
                ? 'hsl(var(--harbor-warning))'
                : 'hsl(var(--harbor-warning) / 0.4)',
              color: 'hsl(var(--harbor-bg-primary))',
            }}
          >
            {part.text}
          </mark>
        ) : (
          <span key={i}>{part.text}</span>
        ),
      )}
    </>
  );
}

// Conversation menu component
function ConversationMenu({
  isOpen,
  onClose,
  onArchive,
  onClearHistory,
  onDelete,
  isArchived,
}: {
  isOpen: boolean;
  onClose: () => void;
  onArchive: () => void;
  onClearHistory: () => void;
  onDelete: () => void;
  isArchived: boolean;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      ref={menuRef}
      className="absolute right-0 top-full mt-1 w-48 rounded-lg shadow-lg z-50 overflow-hidden"
      style={{
        background: 'hsl(var(--harbor-bg-elevated))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      <button
        onClick={() => {
          onArchive();
          onClose();
        }}
        className="w-full px-4 py-3 text-left text-sm flex items-center gap-3 transition-colors hover:bg-white/5"
        style={{ color: 'hsl(var(--harbor-text-primary))' }}
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"
          />
        </svg>
        {isArchived ? 'Unarchive' : 'Archive'}
      </button>
      <button
        onClick={() => {
          onClearHistory();
          onClose();
        }}
        className="w-full px-4 py-3 text-left text-sm flex items-center gap-3 transition-colors hover:bg-white/5"
        style={{ color: 'hsl(var(--harbor-text-primary))' }}
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
          />
        </svg>
        Clear History
      </button>
      <div style={{ borderTop: '1px solid hsl(var(--harbor-border-subtle))' }}>
        <button
          onClick={() => {
            onDelete();
            onClose();
          }}
          className="w-full px-4 py-3 text-left text-sm flex items-center gap-3 transition-colors hover:bg-white/5"
          style={{ color: 'hsl(var(--harbor-error))' }}
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
          Delete Conversation
        </button>
      </div>
    </div>
  );
}

// Confirmation dialog component
function ConfirmDialog({
  isOpen,
  title,
  message,
  confirmLabel,
  confirmDestructive,
  onConfirm,
  onCancel,
}: {
  isOpen: boolean;
  title: string;
  message: string;
  confirmLabel: string;
  confirmDestructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60">
      <div
        className="w-full max-w-sm mx-4 rounded-xl p-6 shadow-2xl"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <h3
          className="text-lg font-bold mb-2"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          {title}
        </h3>
        <p className="text-sm mb-6" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          {message}
        </p>
        <div className="flex gap-3 justify-end">
          <button
            onClick={onCancel}
            className="px-4 py-2 rounded-lg text-sm font-medium transition-colors hover:bg-white/10"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              color: 'hsl(var(--harbor-text-primary))',
            }}
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
            style={{
              background: confirmDestructive
                ? 'hsl(var(--harbor-error))'
                : 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              color: 'white',
            }}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

// Contact picker modal for new conversation
function ContactPicker({
  isOpen,
  onClose,
  onSelect,
  existingPeerIds,
}: {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (peerId: string) => void;
  existingPeerIds: string[];
}) {
  const { contacts, loadContacts } = useContactsStore();
  const [filter, setFilter] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      loadContacts();
      setFilter('');
      setTimeout(() => inputRef.current?.focus(), 100);
    }
  }, [isOpen, loadContacts]);

  if (!isOpen) return null;

  // Filter contacts that don't already have a conversation
  const availableContacts = contacts.filter(
    (c) =>
      !existingPeerIds.includes(c.peerId) &&
      c.displayName.toLowerCase().includes(filter.toLowerCase()),
  );

  // Also show contacts that have existing conversations (for starting a new chat with them)
  const existingContacts = contacts.filter(
    (c) =>
      existingPeerIds.includes(c.peerId) &&
      c.displayName.toLowerCase().includes(filter.toLowerCase()),
  );

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60">
      <div
        className="w-full max-w-md mx-4 rounded-xl shadow-2xl overflow-hidden"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
          maxHeight: '80vh',
        }}
      >
        {/* Header */}
        <div
          className="px-5 py-4 border-b flex items-center justify-between"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <h3
            className="text-lg font-bold"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            New Conversation
          </h3>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg transition-colors hover:bg-white/10"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <XIcon className="w-5 h-5" />
          </button>
        </div>

        {/* Search */}
        <div className="px-5 py-3">
          <div className="relative">
            <SearchIcon
              className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            />
            <input
              ref={inputRef}
              type="text"
              placeholder="Search contacts..."
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              className="w-full pl-10 pr-4 py-2.5 rounded-lg text-sm"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
            />
          </div>
        </div>

        {/* Contact list */}
        <div className="overflow-y-auto px-3 pb-3" style={{ maxHeight: '50vh' }}>
          {contacts.length === 0 ? (
            <div className="text-center py-8">
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                No contacts yet. Add contacts from the Network page.
              </p>
            </div>
          ) : availableContacts.length === 0 && existingContacts.length === 0 ? (
            <div className="text-center py-8">
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                No matching contacts found.
              </p>
            </div>
          ) : (
            <>
              {availableContacts.length > 0 && (
                <div className="space-y-1">
                  {availableContacts.map((contact) => (
                    <button
                      key={contact.peerId}
                      onClick={() => {
                        onSelect(contact.peerId);
                        onClose();
                      }}
                      className="w-full flex items-center gap-3 p-3 rounded-lg transition-colors hover:bg-white/5 text-left"
                    >
                      <div
                        className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white flex-shrink-0"
                        style={{ background: getContactColor(contact.peerId) }}
                      >
                        {getInitials(contact.displayName)}
                      </div>
                      <div className="min-w-0 flex-1">
                        <p
                          className="font-semibold text-sm truncate"
                          style={{ color: 'hsl(var(--harbor-text-primary))' }}
                        >
                          {contact.displayName}
                        </p>
                        <p
                          className="text-xs truncate"
                          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                        >
                          {contact.peerId.slice(0, 16)}...
                        </p>
                      </div>
                      <span
                        className="text-xs px-2 py-1 rounded-full flex-shrink-0"
                        style={{
                          background: 'hsl(var(--harbor-primary) / 0.15)',
                          color: 'hsl(var(--harbor-primary))',
                        }}
                      >
                        New
                      </span>
                    </button>
                  ))}
                </div>
              )}
              {existingContacts.length > 0 && (
                <>
                  {availableContacts.length > 0 && (
                    <p
                      className="text-xs font-medium uppercase tracking-wider px-3 py-2 mt-2"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      Existing conversations
                    </p>
                  )}
                  <div className="space-y-1">
                    {existingContacts.map((contact) => (
                      <button
                        key={contact.peerId}
                        onClick={() => {
                          onSelect(contact.peerId);
                          onClose();
                        }}
                        className="w-full flex items-center gap-3 p-3 rounded-lg transition-colors hover:bg-white/5 text-left"
                      >
                        <div
                          className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white flex-shrink-0"
                          style={{ background: getContactColor(contact.peerId) }}
                        >
                          {getInitials(contact.displayName)}
                        </div>
                        <div className="min-w-0 flex-1">
                          <p
                            className="font-semibold text-sm truncate"
                            style={{ color: 'hsl(var(--harbor-text-primary))' }}
                          >
                            {contact.displayName}
                          </p>
                          <p
                            className="text-xs truncate"
                            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                          >
                            {contact.peerId.slice(0, 16)}...
                          </p>
                        </div>
                      </button>
                    ))}
                  </div>
                </>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// Back arrow icon
function BackIcon({ className }: { className?: string }) {
  return (
    <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
    </svg>
  );
}

// Archive icon
function ArchiveIcon({ className }: { className?: string }) {
  return (
    <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={1.5}
        d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"
      />
    </svg>
  );
}

// Unified conversation type for both real and mock
interface UnifiedConversation {
  id: string;
  peerId: string;
  name: string;
  online: boolean;
  avatarGradient: string;
  lastMessage: string;
  timestamp: Date;
  unread: number;
  isReal: boolean; // true = real contact, false = mock
}

export function ChatPage() {
  const navigate = useNavigate();

  // Real contacts and messaging
  const { contacts, loadContacts } = useContactsStore();
  const {
    conversations: realConversations,
    messages: realMessages,
    loadConversations,
    loadMessages,
    sendMessage: sendRealMessage,
    setActiveConversation,
    selectedConversationId,
    setSelectedConversation,
    clearConversationHistory,
    deleteConversation,
    editMessage,
    archiveConversation,
    unarchiveConversation,
    isArchived,
  } = useMessagingStore();

  // Use store's selectedConversationId
  const selectedConversation = selectedConversationId;

  // Keep the store's activeConversation in sync with local selectedConversation
  // This is needed for the event handler to know which conversation to refresh
  useEffect(() => {
    if (selectedConversation) {
      const peerId = selectedConversation.replace('real-', '');
      setActiveConversation(peerId);
    } else {
      setActiveConversation(null);
    }
  }, [selectedConversation, setActiveConversation]);

  const [searchQuery, setSearchQuery] = useState('');
  const [messageInput, setMessageInput] = useState('');
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [showMessageSearch, setShowMessageSearch] = useState(false);
  const [messageSearchQuery, setMessageSearchQuery] = useState('');
  const [currentSearchIndex, setCurrentSearchIndex] = useState(0);
  const [showContactPicker, setShowContactPicker] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  const [confirmDialog, setConfirmDialog] = useState<{
    isOpen: boolean;
    title: string;
    message: string;
    confirmLabel: string;
    confirmDestructive?: boolean;
    onConfirm: () => void;
  }>({ isOpen: false, title: '', message: '', confirmLabel: '', onConfirm: () => { } });
  const [headerMenuOpen, setHeaderMenuOpen] = useState(false);

  // Attachment state
  const [pendingAttachment, setPendingAttachment] = useState<PendingAttachment | null>(null);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);

  // Edit message state
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');
  const editTextareaRef = useRef<HTMLTextAreaElement>(null);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const messageRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Load real contacts and conversations on mount
  useEffect(() => {
    loadContacts().catch((err) => log.error('Failed to load contacts', err));
    loadConversations().catch((err) => log.error('Failed to load conversations', err));
  }, [loadContacts, loadConversations]);

  // Focus search input when search is shown
  useEffect(() => {
    if (showMessageSearch && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [showMessageSearch]);

  // Reset search index when query changes
  useEffect(() => {
    setCurrentSearchIndex(0);
  }, [messageSearchQuery]);

  // Global Ctrl+F keyboard shortcut to toggle message search
  useEffect(() => {
    const handleGlobalKeyDown = (e: globalThis.KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f' && selectedConversation) {
        e.preventDefault();
        if (!showMessageSearch) {
          setShowMessageSearch(true);
        } else {
          searchInputRef.current?.focus();
          searchInputRef.current?.select();
        }
      }
      if (e.key === 'Escape' && showMessageSearch) {
        setShowMessageSearch(false);
        setMessageSearchQuery('');
      }
    };
    document.addEventListener('keydown', handleGlobalKeyDown);
    return () => document.removeEventListener('keydown', handleGlobalKeyDown);
  }, [selectedConversation, showMessageSearch]);

  // Build conversation list from real contacts
  const unifiedConversations = useMemo<UnifiedConversation[]>(
    () =>
      contacts.map((contact): UnifiedConversation => {
        const realConv = realConversations.find((c) => c.peerId === contact.peerId);
        return {
          id: `real-${contact.peerId}`,
          peerId: contact.peerId,
          name: contact.displayName,
          online: true, // Assume online for now - would need presence tracking
          avatarGradient: getContactColor(contact.peerId),
          lastMessage: realConv ? 'Tap to view messages' : 'Start a conversation',
          timestamp: realConv
            ? new Date(realConv.lastMessageAt * 1000)
            : new Date(contact.addedAt * 1000),
          unread: realConv?.unreadCount || 0,
          isReal: true,
        };
      }),
    [contacts, realConversations],
  );

  // Separate active and archived conversations
  const activeConversations = unifiedConversations.filter((c) => !isArchived(c.peerId));
  const archivedConversationsList = unifiedConversations.filter((c) => isArchived(c.peerId));

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  // Get selected conversation from unified list
  const selectedConv = useMemo(
    () => unifiedConversations.find((c) => c.id === selectedConversation),
    [unifiedConversations, selectedConversation],
  );

  // Get messages for current conversation
  const currentMessages = useMemo(
    () => (selectedConv ? realMessages[selectedConv.peerId] || [] : []),
    [selectedConv, realMessages],
  );

  // Calculate search results
  const searchResults = messageSearchQuery.trim()
    ? currentMessages
      .map((message, index) => ({
        message,
        index,
        content: message.content.toLowerCase(),
      }))
      .filter((item) => item.content.includes(messageSearchQuery.toLowerCase()))
    : [];

  // Navigate search results
  const navigateSearchResults = (direction: 'next' | 'prev') => {
    if (searchResults.length === 0) return;
    setCurrentSearchIndex((prev) => {
      if (direction === 'next') {
        return (prev + 1) % searchResults.length;
      }
      return (prev - 1 + searchResults.length) % searchResults.length;
    });
  };

  // --- Attachment handlers ---
  const handleAttachmentClick = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileSelected = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    // Validate file type
    if (!ALL_ACCEPTED_TYPES.includes(file.type)) {
      toast.error('Unsupported file type. Please select an image (jpg, png, gif, webp) or video (mp4, webm).');
      if (fileInputRef.current) fileInputRef.current.value = '';
      return;
    }

    // Validate file size
    if (file.size > MAX_FILE_SIZE) {
      toast.error(`File is too large (${formatFileSize(file.size)}). Maximum size is 10 MB.`);
      if (fileInputRef.current) fileInputRef.current.value = '';
      return;
    }

    const isVideo = file.type.startsWith('video/');
    const previewUrl = URL.createObjectURL(file);

    setPendingAttachment({
      file,
      type: isVideo ? 'video' : 'image',
      previewUrl,
      name: file.name,
      size: file.size,
    });

    toast.success(`${isVideo ? 'Video' : 'Image'} attached`);

    // Reset file input so the same file can be re-selected
    if (fileInputRef.current) fileInputRef.current.value = '';
  }, []);

  const handleRemoveAttachment = useCallback(() => {
    if (pendingAttachment) {
      URL.revokeObjectURL(pendingAttachment.previewUrl);
      setPendingAttachment(null);
    }
  }, [pendingAttachment]);

  // Clean up object URLs on unmount
  useEffect(() => {
    return () => {
      if (pendingAttachment) {
        URL.revokeObjectURL(pendingAttachment.previewUrl);
      }
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Load messages when selecting a conversation
  useEffect(() => {
    if (selectedConv) {
      loadMessages(selectedConv.peerId).catch((err) => log.error('Failed to load messages', err));
    }
  }, [selectedConv?.peerId, loadMessages]);

  // Scroll to bottom when messages change
  useEffect(() => {
    scrollToBottom();
  }, [currentMessages.length]);

  const filteredConversations = useMemo(
    () =>
      (showArchived ? archivedConversationsList : activeConversations).filter((c) =>
        c.name.toLowerCase().includes(searchQuery.toLowerCase()),
      ),
    [showArchived, archivedConversationsList, activeConversations, searchQuery],
  );

  const handleSendMessage = async () => {
    const hasText = messageInput.trim().length > 0;
    const hasAttachment = pendingAttachment !== null;

    if ((!hasText && !hasAttachment) || !selectedConversation || !selectedConv) return;

    // Build message content: text + optional media marker
    let content = messageInput.trim();
    let contentType = 'text';

    if (hasAttachment) {
      const mediaMarker = `[media:${pendingAttachment!.previewUrl}:${pendingAttachment!.file.type}]`;
      content = content ? `${content}\n${mediaMarker}` : mediaMarker;
      contentType = pendingAttachment!.type;
    }

    setMessageInput('');
    setPendingAttachment(null);
    inputRef.current?.focus();

    try {
      await sendRealMessage(selectedConv.peerId, content, contentType);
      loadMessages(selectedConv.peerId).catch((err) => log.error('Failed to reload messages after send', err));
    } catch (error) {
      log.error('Failed to send message', error);
      toast.error('Failed to send message');
    }
  };

  // Start editing a message
  const handleStartEdit = useCallback((messageId: string, currentContent: string) => {
    setEditingMessageId(messageId);
    setEditContent(currentContent);
    // Focus the edit textarea after render
    setTimeout(() => editTextareaRef.current?.focus(), 50);
  }, []);

  // Save edited message
  const handleSaveEdit = useCallback(async () => {
    if (!editingMessageId || !editContent.trim() || !selectedConv) return;

    try {
      await editMessage(editingMessageId, editContent.trim(), selectedConv.peerId);
      toast.success('Message edited');
    } catch (error) {
      console.error('Failed to edit message:', error);
      toast.error('Failed to edit message');
    } finally {
      setEditingMessageId(null);
      setEditContent('');
    }
  }, [editingMessageId, editContent, selectedConv, editMessage]);

  // Cancel editing
  const handleCancelEdit = useCallback(() => {
    setEditingMessageId(null);
    setEditContent('');
  }, []);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter to send, Shift+Enter for new line
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  const handleEmojiSelect = useCallback(
    (emoji: string) => {
      const textarea = inputRef.current;
      if (textarea) {
        const start = textarea.selectionStart;
        const end = textarea.selectionEnd;
        const before = messageInput.slice(0, start);
        const after = messageInput.slice(end);
        const newValue = before + emoji + after;
        setMessageInput(newValue);

        // Set cursor position after the inserted emoji
        requestAnimationFrame(() => {
          const newPos = start + emoji.length;
          textarea.selectionStart = newPos;
          textarea.selectionEnd = newPos;
          textarea.focus();
        });
      } else {
        // Fallback: append to end
        setMessageInput((prev) => prev + emoji);
      }
    },
    [messageInput],
  );

  const handleCall = () => {
    if (selectedConv) {
      if (selectedConv.online) {
        toast(`Calling ${selectedConv.name}...`, {
          icon: '\u{1F4DE}',
          duration: 3000,
        });
      } else {
        toast.error(`${selectedConv.name} is offline`);
      }
    }
  };

  const handleNewConversation = () => {
    setShowContactPicker(true);
  };

  const handleContactSelected = (peerId: string) => {
    // Navigate to the conversation with this contact
    setSelectedConversation(`real-${peerId}`);
    // If the conversation was archived, unarchive it
    if (isArchived(peerId)) {
      unarchiveConversation(peerId);
    }
  };

  const handleArchive = (conversation: UnifiedConversation) => {
    const archived = isArchived(conversation.peerId);
    if (archived) {
      unarchiveConversation(conversation.peerId);
      toast.success(`Unarchived conversation with ${conversation.name}`);
    } else {
      archiveConversation(conversation.peerId);
      toast.success(`Archived conversation with ${conversation.name}`);
    }
  };

  const handleClearHistory = (conversation: UnifiedConversation) => {
    setConfirmDialog({
      isOpen: true,
      title: 'Clear Chat History',
      message: `Are you sure you want to clear all messages with ${conversation.name}? This action cannot be undone.`,
      confirmLabel: 'Clear History',
      confirmDestructive: true,
      onConfirm: async () => {
        setConfirmDialog((prev) => ({ ...prev, isOpen: false }));
        try {
          await clearConversationHistory(conversation.peerId);
          toast.success(`Chat history with ${conversation.name} cleared`);
        } catch {
          toast.error('Failed to clear chat history');
        }
      },
    });
  };

  const handleDeleteConversation = (conversation: UnifiedConversation) => {
    setConfirmDialog({
      isOpen: true,
      title: 'Delete Conversation',
      message: `Are you sure you want to delete the conversation with ${conversation.name}? All messages will be permanently removed. This action cannot be undone.`,
      confirmLabel: 'Delete',
      confirmDestructive: true,
      onConfirm: async () => {
        setConfirmDialog((prev) => ({ ...prev, isOpen: false }));
        try {
          await deleteConversation(conversation.peerId);
          // If this conversation was selected, deselect it
          if (selectedConversation === conversation.id) {
            setSelectedConversation(null);
          }
          toast.success(`Conversation with ${conversation.name} deleted`);
        } catch {
          toast.error('Failed to delete conversation');
        }
      },
    });
  };

  // Get peer IDs that already have conversations
  const existingConvPeerIds = unifiedConversations.map((c) => c.peerId);

  // Conversation list view (when no conversation selected)
  if (!selectedConversation) {
    return (
      <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
        {/* Header */}
        <div
          className="p-4 border-b flex items-center justify-between"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <div className="flex items-center gap-3">
            <button
              onClick={() => navigate(-1)}
              className="p-2 -ml-2 rounded-lg transition-colors"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              <BackIcon className="w-5 h-5" />
            </button>
            <h2 className="text-lg font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Messages
            </h2>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleNewConversation}
              className="p-2 rounded-lg transition-colors duration-200"
              style={{
                background:
                  'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                color: 'white',
              }}
              title="New conversation"
            >
              <PlusIcon className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Search */}
        <div className="p-4">
          <div className="relative">
            <SearchIcon
              className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            />
            <input
              type="text"
              placeholder="Search conversations..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-10 pr-4 py-3 rounded-lg text-sm"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
            />
          </div>
        </div>

        {/* Active / Archived toggle */}
        {archivedConversationsList.length > 0 && (
          <div className="px-4 pb-2 flex gap-2">
            <button
              onClick={() => setShowArchived(false)}
              className="px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
              style={{
                background: !showArchived
                  ? 'hsl(var(--harbor-primary) / 0.15)'
                  : 'hsl(var(--harbor-surface-1))',
                color: !showArchived
                  ? 'hsl(var(--harbor-primary))'
                  : 'hsl(var(--harbor-text-secondary))',
              }}
            >
              Active ({activeConversations.length})
            </button>
            <button
              onClick={() => setShowArchived(true)}
              className="px-3 py-1.5 rounded-lg text-xs font-medium transition-colors flex items-center gap-1.5"
              style={{
                background: showArchived
                  ? 'hsl(var(--harbor-primary) / 0.15)'
                  : 'hsl(var(--harbor-surface-1))',
                color: showArchived
                  ? 'hsl(var(--harbor-primary))'
                  : 'hsl(var(--harbor-text-secondary))',
              }}
            >
              <ArchiveIcon className="w-3.5 h-3.5" />
              Archived ({archivedConversationsList.length})
            </button>
          </div>
        )}

        {/* Conversation list */}
        <div className="flex-1 overflow-y-auto px-2">
          {filteredConversations.length === 0 ? (
            <div className="text-center py-12">
              <ChatIcon
                className="w-16 h-16 mx-auto mb-4"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              />
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                {showArchived
                  ? 'No archived conversations'
                  : 'No conversations found'}
              </p>
              {!showArchived && contacts.length > 0 && (
                <button
                  onClick={handleNewConversation}
                  className="mt-4 px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                  }}
                >
                  Start a new conversation
                </button>
              )}
            </div>
          ) : (
            <div className="space-y-1">
              {filteredConversations.map((conversation) => (
                <div
                  key={conversation.id}
                  className="relative flex items-center rounded-lg transition-all duration-200 hover:bg-white/5"
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setOpenMenuId(openMenuId === conversation.id ? null : conversation.id);
                  }}
                >
                  <button
                    onClick={() => setSelectedConversation(conversation.id)}
                    className="flex-1 flex items-center gap-3 p-3 text-left"
                  >
                    {/* Avatar */}
                    <div className="relative flex-shrink-0">
                      <div
                        className="w-12 h-12 rounded-full flex items-center justify-center text-sm font-semibold text-white"
                        style={{
                          background: conversation.avatarGradient,
                        }}
                      >
                        {getInitials(conversation.name)}
                      </div>
                      {conversation.online && (
                        <div
                          className="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 rounded-full border-2"
                          style={{
                            background: 'hsl(var(--harbor-success))',
                            borderColor: 'hsl(var(--harbor-bg-primary))',
                          }}
                        />
                      )}
                    </div>

                    {/* Info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center justify-between mb-0.5">
                        <div className="flex items-center gap-2 min-w-0">
                          <p
                            className="font-semibold text-sm truncate"
                            style={{ color: 'hsl(var(--harbor-text-primary))' }}
                          >
                            {conversation.name}
                          </p>
                          <span
                            className="text-[10px] px-1.5 py-0.5 rounded flex-shrink-0"
                            style={{
                              background: 'hsl(var(--harbor-success) / 0.15)',
                              color: 'hsl(var(--harbor-success))',
                            }}
                          >
                            P2P
                          </span>
                          {isArchived(conversation.peerId) && (
                            <span
                              className="text-[10px] px-1.5 py-0.5 rounded flex-shrink-0"
                              style={{
                                background: 'hsl(var(--harbor-text-tertiary) / 0.15)',
                                color: 'hsl(var(--harbor-text-tertiary))',
                              }}
                            >
                              Archived
                            </span>
                          )}
                        </div>
                        <span
                          className="text-xs flex-shrink-0 ml-2"
                          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                        >
                          {formatRelativeTime(conversation.timestamp)}
                        </span>
                      </div>
                      <div className="flex items-center justify-between">
                        <p
                          className="text-sm truncate"
                          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                        >
                          {conversation.lastMessage}
                        </p>
                        {conversation.unread > 0 && (
                          <span
                            className="ml-2 px-2 py-0.5 rounded-full text-xs font-semibold flex-shrink-0"
                            style={{
                              background:
                                'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                              color: 'white',
                            }}
                          >
                            {conversation.unread}
                          </span>
                        )}
                      </div>
                    </div>
                  </button>

                  {/* Three-dot menu button */}
                  <div className="relative pr-2">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setOpenMenuId(openMenuId === conversation.id ? null : conversation.id);
                      }}
                      className="p-2 rounded-lg transition-colors duration-200 hover:bg-white/10"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      <EllipsisIcon className="w-5 h-5" />
                    </button>
                    <ConversationMenu
                      isOpen={openMenuId === conversation.id}
                      onClose={() => setOpenMenuId(null)}
                      isArchived={isArchived(conversation.peerId)}
                      onArchive={() => handleArchive(conversation)}
                      onClearHistory={() => handleClearHistory(conversation)}
                      onDelete={() => handleDeleteConversation(conversation)}
                    />
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Contact picker modal */}
        <ContactPicker
          isOpen={showContactPicker}
          onClose={() => setShowContactPicker(false)}
          onSelect={handleContactSelected}
          existingPeerIds={existingConvPeerIds}
        />

        {/* Confirmation dialog */}
        <ConfirmDialog
          isOpen={confirmDialog.isOpen}
          title={confirmDialog.title}
          message={confirmDialog.message}
          confirmLabel={confirmDialog.confirmLabel}
          confirmDestructive={confirmDialog.confirmDestructive}
          onConfirm={confirmDialog.onConfirm}
          onCancel={() => setConfirmDialog((prev) => ({ ...prev, isOpen: false }))}
        />
      </div>
    );
  }

  // Chat view (when conversation selected)
  return (
    <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Chat header */}
      <header
        className="px-4 py-3 border-b flex items-center justify-between flex-shrink-0"
        style={{
          borderColor: 'hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-bg-elevated))',
        }}
      >
        <div className="flex items-center gap-3">
          <button
            onClick={() => setSelectedConversation(null)}
            className="p-2 -ml-2 rounded-lg transition-colors"
            style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          >
            <BackIcon className="w-5 h-5" />
          </button>
          <div className="relative">
            <div
              className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white"
              style={{
                background: selectedConv!.avatarGradient,
              }}
            >
              {getInitials(selectedConv!.name)}
            </div>
            {selectedConv!.online && (
              <div
                className="absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2"
                style={{
                  background: 'hsl(var(--harbor-success))',
                  borderColor: 'hsl(var(--harbor-bg-elevated))',
                }}
              />
            )}
          </div>
          <div>
            <p className="font-semibold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              {selectedConv!.name}
            </p>
            <p
              className="text-xs"
              style={{
                color: selectedConv!.online
                  ? 'hsl(var(--harbor-success))'
                  : 'hsl(var(--harbor-text-tertiary))',
              }}
            >
              {selectedConv!.isReal
                ? selectedConv!.online
                  ? 'Online'
                  : 'Offline'
                : selectedConv!.online
                  ? 'Online - will reply automatically'
                  : 'Offline'}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => {
              setShowMessageSearch(!showMessageSearch);
              if (showMessageSearch) {
                setMessageSearchQuery('');
              }
            }}
            className="p-2 rounded-lg transition-colors duration-200"
            style={{
              background: showMessageSearch
                ? 'hsl(var(--harbor-primary) / 0.15)'
                : 'hsl(var(--harbor-surface-1))',
              color: showMessageSearch
                ? 'hsl(var(--harbor-primary))'
                : 'hsl(var(--harbor-text-secondary))',
            }}
            title="Search messages (Ctrl+F)"
          >
            <SearchIcon className="w-5 h-5" />
          </button>
          <button
            onClick={handleCall}
            className="p-2 rounded-lg transition-colors duration-200"
            style={{
              background: 'hsl(var(--harbor-success) / 0.15)',
              color: 'hsl(var(--harbor-success))',
            }}
          >
            <PhoneIcon className="w-5 h-5" />
          </button>
          <div className="relative">
            <button
              onClick={() => setHeaderMenuOpen(!headerMenuOpen)}
              className="p-2 rounded-lg transition-colors duration-200"
              style={{
                background: headerMenuOpen
                  ? 'hsl(var(--harbor-primary) / 0.15)'
                  : 'hsl(var(--harbor-surface-1))',
                color: headerMenuOpen
                  ? 'hsl(var(--harbor-primary))'
                  : 'hsl(var(--harbor-text-secondary))',
              }}
            >
              <EllipsisIcon className="w-5 h-5" />
            </button>
            <ConversationMenu
              isOpen={headerMenuOpen}
              onClose={() => setHeaderMenuOpen(false)}
              isArchived={isArchived(selectedConv!.peerId)}
              onArchive={() => handleArchive(selectedConv!)}
              onClearHistory={() => handleClearHistory(selectedConv!)}
              onDelete={() => handleDeleteConversation(selectedConv!)}
            />
          </div>
        </div>
      </header>

      {/* Message search bar */}
      {showMessageSearch && (
        <div
          className="px-4 py-2 border-b flex items-center gap-3"
          style={{
            borderColor: 'hsl(var(--harbor-border-subtle))',
            background: 'hsl(var(--harbor-bg-elevated))',
          }}
        >
          <div className="flex-1 relative">
            <SearchIcon
              className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            />
            <input
              ref={searchInputRef}
              type="text"
              placeholder="Search messages..."
              value={messageSearchQuery}
              onChange={(e) => setMessageSearchQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  navigateSearchResults(e.shiftKey ? 'prev' : 'next');
                }
                if (e.key === 'Escape') {
                  setShowMessageSearch(false);
                  setMessageSearchQuery('');
                }
              }}
              className="w-full pl-10 pr-4 py-2 rounded-lg text-sm"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
            />
          </div>
          {messageSearchQuery.trim() && (
            <div className="flex items-center gap-2 flex-shrink-0">
              <span
                className="text-sm whitespace-nowrap"
                style={{
                  color:
                    searchResults.length > 0
                      ? 'hsl(var(--harbor-text-secondary))'
                      : 'hsl(var(--harbor-error))',
                }}
              >
                {searchResults.length > 0
                  ? `${currentSearchIndex + 1} of ${searchResults.length}`
                  : 'No results'}
              </span>
              {searchResults.length > 0 && (
                <>
                  <button
                    onClick={() => navigateSearchResults('prev')}
                    className="p-1.5 rounded-lg transition-colors hover:bg-white/10"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-secondary))',
                    }}
                    title="Previous match (Shift+Enter)"
                  >
                    <ChevronUpIcon className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => navigateSearchResults('next')}
                    className="p-1.5 rounded-lg transition-colors hover:bg-white/10"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-secondary))',
                    }}
                    title="Next match (Enter)"
                  >
                    <ChevronDownIcon className="w-4 h-4" />
                  </button>
                </>
              )}
            </div>
          )}
          <button
            onClick={() => {
              setShowMessageSearch(false);
              setMessageSearchQuery('');
            }}
            className="p-1.5 rounded-lg transition-colors"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <XIcon className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="max-w-3xl mx-auto space-y-3">
          {currentMessages.map((message, messageIndex) => {
            const isMine = message.isOutgoing;
            const timestamp = new Date(message.sentAt * 1000);
            const content = message.content;
            const id = message.messageId;
            const isMatch = showMessageSearch && matchingMessageIndices.has(messageIndex);
            const isActiveMatch = messageIndex === activeMatchMessageIndex;
            const contentHasMedia = hasMediaContent(content);
            const segments = contentHasMedia ? parseMessageContent(content) : null;
            const isEditing = editingMessageId === id;
            const isEdited = message.editedAt != null;

            return (
              <div
                key={id}
                ref={(el) => {
                  if (el) {
                    messageRefs.current.set(messageIndex, el);
                  } else {
                    messageRefs.current.delete(messageIndex);
                  }
                }}
                className={`group flex ${isMine ? 'justify-end' : 'justify-start'}`}
              >
                {/* Edit button for own messages (appears on hover, before the bubble) */}
                {isMine && !isEditing && (
                  <button
                    onClick={() => handleStartEdit(id, content)}
                    className="self-center mr-2 p-1.5 rounded-lg opacity-0 group-hover:opacity-100 transition-opacity duration-150"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-tertiary))',
                    }}
                    title="Edit message"
                  >
                    <PencilIcon size={14} />
                  </button>
                )}

                <div
                  className={`max-w-[75%] rounded-2xl transition-shadow duration-200 overflow-hidden ${contentHasMedia && !isEditing ? 'p-1.5' : 'px-4 py-2.5'}`}
                  style={{
                    background: isMine
                      ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                      : 'hsl(var(--harbor-surface-1))',
                    color: isMine ? 'white' : 'hsl(var(--harbor-text-primary))',
                    borderBottomRightRadius: isMine ? '4px' : '16px',
                    borderBottomLeftRadius: isMine ? '16px' : '4px',
                    boxShadow: isActiveMatch
                      ? '0 0 0 2px hsl(var(--harbor-warning)), 0 0 12px hsl(var(--harbor-warning) / 0.3)'
                      : isMatch
                        ? '0 0 0 1px hsl(var(--harbor-warning) / 0.5)'
                        : 'none',
                  }}
                >
                  {isEditing ? (
                    /* Edit mode: inline textarea with save/cancel */
                    <div className="space-y-2">
                      <textarea
                        ref={editTextareaRef}
                        value={editContent}
                        onChange={(e) => setEditContent(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' && !e.shiftKey) {
                            e.preventDefault();
                            handleSaveEdit();
                          }
                          if (e.key === 'Escape') {
                            handleCancelEdit();
                          }
                        }}
                        rows={Math.min(editContent.split('\n').length, 5)}
                        className="w-full text-sm rounded-lg px-3 py-2 resize-none"
                        style={{
                          background: 'rgba(255,255,255,0.15)',
                          border: '1px solid rgba(255,255,255,0.3)',
                          color: isMine ? 'white' : 'hsl(var(--harbor-text-primary))',
                          outline: 'none',
                          minWidth: '200px',
                        }}
                      />
                      <div className="flex items-center justify-between">
                        <span
                          className="text-xs"
                          style={{
                            color: isMine ? 'rgba(255,255,255,0.6)' : 'hsl(var(--harbor-text-tertiary))',
                          }}
                        >
                          Esc to cancel
                        </span>
                        <div className="flex gap-1.5">
                          <button
                            onClick={handleCancelEdit}
                            className="px-2.5 py-1 rounded-md text-xs font-medium transition-colors"
                            style={{
                              background: 'rgba(255,255,255,0.15)',
                              color: isMine ? 'rgba(255,255,255,0.8)' : 'hsl(var(--harbor-text-secondary))',
                            }}
                          >
                            Cancel
                          </button>
                          <button
                            onClick={handleSaveEdit}
                            disabled={!editContent.trim() || editContent.trim() === content}
                            className="px-2.5 py-1 rounded-md text-xs font-medium transition-colors flex items-center gap-1"
                            style={{
                              background:
                                editContent.trim() && editContent.trim() !== content
                                  ? 'rgba(255,255,255,0.25)'
                                  : 'rgba(255,255,255,0.1)',
                              color:
                                editContent.trim() && editContent.trim() !== content
                                  ? isMine ? 'white' : 'hsl(var(--harbor-text-primary))'
                                  : isMine ? 'rgba(255,255,255,0.4)' : 'hsl(var(--harbor-text-tertiary))',
                            }}
                          >
                            <CheckIcon size={12} />
                            Save
                          </button>
                        </div>
                      </div>
                    </div>
                  ) : contentHasMedia && segments ? (
                    <div className="space-y-1.5">
                      {segments.map((segment, segIdx) =>
                        segment.type === 'media' ? (
                          <ChatMediaDisplay
                            key={segIdx}
                            url={segment.value}
                            mimeType={segment.mimeType || 'image/jpeg'}
                            isMine={isMine}
                          />
                        ) : (
                          <p key={segIdx} className="text-sm whitespace-pre-wrap px-2.5 py-1">
                            {isMatch ? (
                              <HighlightedText
                                text={segment.value}
                                query={messageSearchQuery}
                                isActiveMatch={isActiveMatch}
                              />
                            ) : (
                              segment.value
                            )}
                          </p>
                        ),
                      )}
                      <p
                        className="text-xs text-right px-2.5 pb-1"
                        style={{
                          color: isMine ? 'rgba(255,255,255,0.7)' : 'hsl(var(--harbor-text-tertiary))',
                        }}
                      >
                        {isEdited && (
                          <span className="mr-1" style={{ opacity: 0.7 }}>(edited)</span>
                        )}
                        {timestamp.toLocaleTimeString([], {
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </p>
                    </div>
                  ) : (
                    <>
                      <p className="text-sm whitespace-pre-wrap">
                        {isMatch ? (
                          <HighlightedText
                            text={content}
                            query={messageSearchQuery}
                            isActiveMatch={isActiveMatch}
                          />
                        ) : (
                          content
                        )}
                      </p>
                      <p
                        className="text-xs mt-1 text-right"
                        style={{
                          color: isMine ? 'rgba(255,255,255,0.7)' : 'hsl(var(--harbor-text-tertiary))',
                        }}
                      >
                        {isEdited && (
                          <span className="mr-1" style={{ opacity: 0.7 }}>(edited)</span>
                        )}
                        {timestamp.toLocaleTimeString([], {
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </p>
                    </>
                  )}
                </div>
              </div>
            );
          })}
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Hidden file input for attachments */}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/jpeg,image/png,image/gif,image/webp,video/mp4,video/webm"
        className="hidden"
        onChange={handleFileSelected}
      />

      {/* Message input */}
      <div
        className="p-4 border-t"
        style={{
          borderColor: 'hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-bg-elevated))',
        }}
      >
        <div className="max-w-3xl mx-auto">
          {/* Attachment preview */}
          {pendingAttachment && (
            <div
              className="mb-3 p-3 rounded-lg flex items-start gap-3"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <div className="relative flex-shrink-0 rounded-lg overflow-hidden">
                {pendingAttachment.type === 'image' ? (
                  <img
                    src={pendingAttachment.previewUrl}
                    alt={pendingAttachment.name}
                    className="w-20 h-20 object-cover rounded-lg"
                  />
                ) : (
                  <video
                    src={pendingAttachment.previewUrl}
                    className="w-20 h-20 object-cover rounded-lg"
                    preload="metadata"
                  />
                )}
                {pendingAttachment.type === 'video' && (
                  <div className="absolute inset-0 flex items-center justify-center">
                    <div
                      className="w-8 h-8 rounded-full flex items-center justify-center"
                      style={{ background: 'rgba(0,0,0,0.5)' }}
                    >
                      <svg className="w-4 h-4 text-white ml-0.5" fill="currentColor" viewBox="0 0 24 24">
                        <path d="M8 5v14l11-7z" />
                      </svg>
                    </div>
                  </div>
                )}
              </div>
              <div className="flex-1 min-w-0">
                <p
                  className="text-sm font-medium truncate"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {pendingAttachment.name}
                </p>
                <p
                  className="text-xs mt-0.5"
                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                >
                  {formatFileSize(pendingAttachment.size)} -- {pendingAttachment.type === 'image' ? 'Image' : 'Video'}
                </p>
              </div>
              <button
                onClick={handleRemoveAttachment}
                className="flex-shrink-0 p-1.5 rounded-lg transition-colors hover:bg-white/10"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                title="Remove attachment"
              >
                <XIcon className="w-4 h-4" />
              </button>
            </div>
          )}

          {/* Input row */}
          <div className="flex items-end gap-2">
            <button
              onClick={handleAttachmentClick}
              className="p-3 rounded-lg transition-colors duration-200 flex-shrink-0 hover:bg-white/5"
              style={{
                background: pendingAttachment
                  ? 'hsl(var(--harbor-primary) / 0.15)'
                  : 'hsl(var(--harbor-surface-1))',
                color: pendingAttachment
                  ? 'hsl(var(--harbor-primary))'
                  : 'hsl(var(--harbor-text-secondary))',
              }}
              title="Attach image or video"
            >
              <PaperclipIcon className="w-5 h-5" />
            </button>
            <textarea
              ref={inputRef}
              placeholder={pendingAttachment ? 'Add a caption...' : 'Type a message...'}
              value={messageInput}
              onChange={(e) => setMessageInput(e.target.value)}
              onKeyDown={handleKeyDown}
              rows={1}
              className="flex-1 px-4 py-3 rounded-lg text-sm resize-none max-h-32"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
            />
            {/* Emoji picker button */}
            <div className="relative flex-shrink-0">
              <button
                onClick={() => setShowEmojiPicker(!showEmojiPicker)}
                className="p-3 rounded-lg transition-colors duration-200 hover:bg-white/5"
                style={{
                  background: showEmojiPicker
                    ? 'hsl(var(--harbor-primary) / 0.15)'
                    : 'hsl(var(--harbor-surface-1))',
                  color: showEmojiPicker
                    ? 'hsl(var(--harbor-primary))'
                    : 'hsl(var(--harbor-text-secondary))',
                }}
                title="Insert emoji"
                type="button"
              >
                <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M15.182 15.182a4.5 4.5 0 01-6.364 0M21 12a9 9 0 11-18 0 9 9 0 0118 0zM9.75 9.75c0 .414-.168.75-.375.75S9 10.164 9 9.75 9.168 9 9.375 9s.375.336.375.75zm-.375 0h.008v.015h-.008V9.75zm5.625 0c0 .414-.168.75-.375.75s-.375-.336-.375-.75.168-.75.375-.75.375.336.375.75zm-.375 0h.008v.015h-.008V9.75z" />
                </svg>
              </button>
              {showEmojiPicker && (
                <EmojiPicker
                  onSelect={handleEmojiSelect}
                  onClose={() => setShowEmojiPicker(false)}
                />
              )}
            </div>
            <button
              onClick={handleSendMessage}
              disabled={!messageInput.trim() && !pendingAttachment}
              className="p-3 rounded-lg transition-all duration-200 flex-shrink-0"
              style={{
                background: (messageInput.trim() || pendingAttachment)
                  ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                  : 'hsl(var(--harbor-surface-2))',
                color: (messageInput.trim() || pendingAttachment) ? 'white' : 'hsl(var(--harbor-text-tertiary))',
                boxShadow: (messageInput.trim() || pendingAttachment)
                  ? '0 4px 12px hsl(var(--harbor-primary) / 0.3)'
                  : 'none',
              }}
            >
              <SendIcon className="w-5 h-5" />
            </button>
          </div>
        </div>
        <p
          className="text-xs mt-2 text-center"
          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
        >
          Press Enter to send &bull;{' '}
          {selectedConv?.isReal
            ? 'End-to-end encrypted'
            : selectedConv?.online
              ? 'Demo mode - auto replies enabled'
              : 'Demo peer is offline'}
        </p>
      </div>

      {/* Confirmation dialog */}
      <ConfirmDialog
        isOpen={confirmDialog.isOpen}
        title={confirmDialog.title}
        message={confirmDialog.message}
        confirmLabel={confirmDialog.confirmLabel}
        confirmDestructive={confirmDialog.confirmDestructive}
        onConfirm={confirmDialog.onConfirm}
        onCancel={() => setConfirmDialog((prev) => ({ ...prev, isOpen: false }))}
      />
    </div>
  );
}
