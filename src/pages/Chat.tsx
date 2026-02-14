import { useState, useMemo, useRef, useEffect, KeyboardEvent } from 'react';
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
} from '../components/icons';
import { useContactsStore, useMessagingStore } from '../stores';
import { getInitials, getContactColor, formatRelativeTime } from '../utils/formatting';

const log = createLogger('Chat');

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

// Back arrow icon
function BackIcon({ className }: { className?: string }) {
  return (
    <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
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
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

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
  const searchResults = useMemo(
    () =>
      messageSearchQuery.trim()
        ? currentMessages
            .map((message, index) => ({
              message,
              index,
              content: message.content.toLowerCase(),
            }))
            .filter((item) => item.content.includes(messageSearchQuery.toLowerCase()))
        : [],
    [messageSearchQuery, currentMessages],
  );

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
      unifiedConversations.filter((c) => c.name.toLowerCase().includes(searchQuery.toLowerCase())),
    [unifiedConversations, searchQuery],
  );

  const handleSendMessage = async () => {
    if (!messageInput.trim() || !selectedConversation || !selectedConv) return;

    const content = messageInput.trim();
    setMessageInput('');
    inputRef.current?.focus();

    try {
      await sendRealMessage(selectedConv.peerId, content);
      loadMessages(selectedConv.peerId).catch((err) => log.error('Failed to reload messages after send', err));
    } catch (error) {
      log.error('Failed to send message', error);
      toast.error('Failed to send message');
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter to send, Shift+Enter for new line
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  const handleCall = () => {
    if (selectedConv) {
      if (selectedConv.online) {
        toast(`Calling ${selectedConv.name}...`, {
          icon: '📞',
          duration: 3000,
        });
      } else {
        toast.error(`${selectedConv.name} is offline`);
      }
    }
  };

  const handleNewConversation = () => {
    toast('Select a contact from Network to start a conversation', {
      icon: '💬',
    });
  };

  const handleConversationMenu = () => {
    if (selectedConv) {
      toast(`Options for ${selectedConv.name}`, {
        icon: '📋',
      });
    }
  };

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

        {/* Conversation list */}
        <div className="flex-1 overflow-y-auto px-2">
          {filteredConversations.length === 0 ? (
            <div className="text-center py-12">
              <ChatIcon
                className="w-16 h-16 mx-auto mb-4"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              />
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                No conversations found
              </p>
            </div>
          ) : (
            <div className="space-y-1">
              {filteredConversations.map((conversation) => (
                <div
                  key={conversation.id}
                  className="relative flex items-center rounded-lg transition-all duration-200 hover:bg-white/5"
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

                  {/* Menu button - only for mock conversations */}
                  {!conversation.isReal && (
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
                        isArchived={false}
                        onArchive={() => {
                          toast('Archive coming soon');
                        }}
                        onClearHistory={() => {
                          toast('Clear history coming soon');
                        }}
                        onDelete={() => {
                          toast('Delete coming soon');
                        }}
                      />
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
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
          <button
            onClick={handleConversationMenu}
            className="p-2 rounded-lg transition-colors duration-200"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              color: 'hsl(var(--harbor-text-secondary))',
            }}
          >
            <EllipsisIcon className="w-5 h-5" />
          </button>
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
          {searchResults.length > 0 && (
            <div className="flex items-center gap-2">
              <span className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {currentSearchIndex + 1} of {searchResults.length}
              </span>
              <button
                onClick={() => navigateSearchResults('prev')}
                className="p-1.5 rounded-lg transition-colors"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-secondary))',
                }}
              >
                <ChevronUpIcon className="w-4 h-4" />
              </button>
              <button
                onClick={() => navigateSearchResults('next')}
                className="p-1.5 rounded-lg transition-colors"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-secondary))',
                }}
              >
                <ChevronDownIcon className="w-4 h-4" />
              </button>
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
          {currentMessages.map((message) => {
            const isMine = message.isOutgoing;
            const timestamp = new Date(message.sentAt * 1000);
            const content = message.content;
            const id = message.messageId;

            return (
              <div key={id} className={`flex ${isMine ? 'justify-end' : 'justify-start'}`}>
                <div
                  className="max-w-[75%] px-4 py-2.5 rounded-2xl"
                  style={{
                    background: isMine
                      ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                      : 'hsl(var(--harbor-surface-1))',
                    color: isMine ? 'white' : 'hsl(var(--harbor-text-primary))',
                    borderBottomRightRadius: isMine ? '4px' : '16px',
                    borderBottomLeftRadius: isMine ? '16px' : '4px',
                  }}
                >
                  <p className="text-sm whitespace-pre-wrap">{content}</p>
                  <p
                    className="text-xs mt-1 text-right"
                    style={{
                      color: isMine ? 'rgba(255,255,255,0.7)' : 'hsl(var(--harbor-text-tertiary))',
                    }}
                  >
                    {timestamp.toLocaleTimeString([], {
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                  </p>
                </div>
              </div>
            );
          })}
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Message input */}
      <div
        className="p-4 border-t"
        style={{
          borderColor: 'hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-bg-elevated))',
        }}
      >
        <div className="max-w-3xl mx-auto flex items-end gap-3">
          <textarea
            ref={inputRef}
            placeholder="Type a message..."
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
          <button
            onClick={handleSendMessage}
            disabled={!messageInput.trim()}
            className="p-3 rounded-lg transition-all duration-200 flex-shrink-0"
            style={{
              background: messageInput.trim()
                ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                : 'hsl(var(--harbor-surface-2))',
              color: messageInput.trim() ? 'white' : 'hsl(var(--harbor-text-tertiary))',
              boxShadow: messageInput.trim()
                ? '0 4px 12px hsl(var(--harbor-primary) / 0.3)'
                : 'none',
            }}
          >
            <SendIcon className="w-5 h-5" />
          </button>
        </div>
        <p
          className="text-xs mt-2 text-center"
          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
        >
          Press Enter to send •{' '}
          {selectedConv?.isReal
            ? 'End-to-end encrypted'
            : selectedConv?.online
              ? 'Demo mode - auto replies enabled'
              : 'Demo peer is offline'}
        </p>
      </div>
    </div>
  );
}
