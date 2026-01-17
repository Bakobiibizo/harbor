import { useState, useRef, useEffect, KeyboardEvent } from "react";
import { useNavigate } from "react-router-dom";
import toast from "react-hot-toast";
import {
  ChatIcon,
  SearchIcon,
  PlusIcon,
  SendIcon,
  PhoneIcon,
  EllipsisIcon,
} from "../components/icons";
import { useMockPeersStore } from "../stores";

// Back arrow icon
function BackIcon({ className }: { className?: string }) {
  return (
    <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
    </svg>
  );
}

export function ChatPage() {
  const navigate = useNavigate();
  const { conversations, sendMessage } = useMockPeersStore();
  const [selectedConversation, setSelectedConversation] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [messageInput, setMessageInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  // Scroll to bottom when messages change
  const selectedConv = conversations.find((c) => c.id === selectedConversation);
  useEffect(() => {
    scrollToBottom();
  }, [selectedConv?.messages.length]);

  const formatTime = (date: Date) => {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const mins = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (mins < 1) return "now";
    if (mins < 60) return `${mins}m`;
    if (hours < 24) return `${hours}h`;
    return `${days}d`;
  };

  const getInitials = (name: string) => {
    return name
      .split(" ")
      .map((n) => n[0])
      .join("")
      .toUpperCase()
      .slice(0, 2);
  };

  const filteredConversations = conversations.filter((c) =>
    c.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleSendMessage = () => {
    if (!messageInput.trim() || !selectedConversation) return;

    // Send message through the store (this will trigger auto-reply for online peers)
    sendMessage(selectedConversation, messageInput.trim());
    setMessageInput("");
    inputRef.current?.focus();
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter to send, Shift+Enter for new line
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  const handleCall = () => {
    if (selectedConv) {
      if (selectedConv.online) {
        toast(`Calling ${selectedConv.name}...`, {
          icon: "📞",
          duration: 3000,
        });
      } else {
        toast.error(`${selectedConv.name} is offline`);
      }
    }
  };

  const handleNewConversation = () => {
    toast("Select a contact from Network to start a conversation", {
      icon: "💬",
    });
  };

  const handleConversationMenu = () => {
    if (selectedConv) {
      toast(`Options for ${selectedConv.name}`, {
        icon: "📋",
      });
    }
  };

  // Conversation list view (when no conversation selected)
  if (!selectedConversation) {
    return (
      <div
        className="h-full flex flex-col"
        style={{ background: "hsl(var(--harbor-bg-primary))" }}
      >
        {/* Header */}
        <div
          className="p-4 border-b flex items-center justify-between"
          style={{ borderColor: "hsl(var(--harbor-border-subtle))" }}
        >
          <div className="flex items-center gap-3">
            <button
              onClick={() => navigate(-1)}
              className="p-2 -ml-2 rounded-lg transition-colors"
              style={{ color: "hsl(var(--harbor-text-secondary))" }}
            >
              <BackIcon className="w-5 h-5" />
            </button>
            <h2
              className="text-lg font-bold"
              style={{ color: "hsl(var(--harbor-text-primary))" }}
            >
              Messages
            </h2>
          </div>
          <button
            onClick={handleNewConversation}
            className="p-2 rounded-lg transition-colors duration-200"
            style={{
              background: "linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))",
              color: "white",
            }}
          >
            <PlusIcon className="w-5 h-5" />
          </button>
        </div>

        {/* Search */}
        <div className="p-4">
          <div className="relative">
            <SearchIcon
              className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
              style={{ color: "hsl(var(--harbor-text-tertiary))" }}
            />
            <input
              type="text"
              placeholder="Search conversations..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-10 pr-4 py-3 rounded-lg text-sm"
              style={{
                background: "hsl(var(--harbor-surface-1))",
                border: "1px solid hsl(var(--harbor-border-subtle))",
                color: "hsl(var(--harbor-text-primary))",
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
                style={{ color: "hsl(var(--harbor-text-tertiary))" }}
              />
              <p
                className="text-sm"
                style={{ color: "hsl(var(--harbor-text-tertiary))" }}
              >
                No conversations found
              </p>
            </div>
          ) : (
            <div className="space-y-1">
              {filteredConversations.map((conversation) => (
                <button
                  key={conversation.id}
                  onClick={() => setSelectedConversation(conversation.id)}
                  className="w-full flex items-center gap-3 p-3 rounded-lg text-left transition-all duration-200"
                  style={{
                    background: "transparent",
                  }}
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
                          background: "hsl(var(--harbor-success))",
                          borderColor: "hsl(var(--harbor-bg-primary))",
                        }}
                      />
                    )}
                  </div>

                  {/* Info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between mb-0.5">
                      <p
                        className="font-semibold text-sm truncate"
                        style={{ color: "hsl(var(--harbor-text-primary))" }}
                      >
                        {conversation.name}
                      </p>
                      <span
                        className="text-xs flex-shrink-0 ml-2"
                        style={{ color: "hsl(var(--harbor-text-tertiary))" }}
                      >
                        {formatTime(conversation.timestamp)}
                      </span>
                    </div>
                    <div className="flex items-center justify-between">
                      <p
                        className="text-sm truncate"
                        style={{ color: "hsl(var(--harbor-text-secondary))" }}
                      >
                        {conversation.lastMessage}
                      </p>
                      {conversation.unread > 0 && (
                        <span
                          className="ml-2 px-2 py-0.5 rounded-full text-xs font-semibold flex-shrink-0"
                          style={{
                            background: "linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))",
                            color: "white",
                          }}
                        >
                          {conversation.unread}
                        </span>
                      )}
                    </div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }

  // Chat view (when conversation selected)
  return (
    <div
      className="h-full flex flex-col"
      style={{ background: "hsl(var(--harbor-bg-primary))" }}
    >
      {/* Chat header */}
      <header
        className="px-4 py-3 border-b flex items-center justify-between flex-shrink-0"
        style={{
          borderColor: "hsl(var(--harbor-border-subtle))",
          background: "hsl(var(--harbor-bg-elevated))",
        }}
      >
        <div className="flex items-center gap-3">
          <button
            onClick={() => setSelectedConversation(null)}
            className="p-2 -ml-2 rounded-lg transition-colors"
            style={{ color: "hsl(var(--harbor-text-secondary))" }}
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
                  background: "hsl(var(--harbor-success))",
                  borderColor: "hsl(var(--harbor-bg-elevated))",
                }}
              />
            )}
          </div>
          <div>
            <p
              className="font-semibold"
              style={{ color: "hsl(var(--harbor-text-primary))" }}
            >
              {selectedConv!.name}
            </p>
            <p
              className="text-xs"
              style={{
                color: selectedConv!.online
                  ? "hsl(var(--harbor-success))"
                  : "hsl(var(--harbor-text-tertiary))",
              }}
            >
              {selectedConv!.online ? "Online - will reply automatically" : "Offline"}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={handleCall}
            className="p-2 rounded-lg transition-colors duration-200"
            style={{
              background: "hsl(var(--harbor-success) / 0.15)",
              color: "hsl(var(--harbor-success))",
            }}
          >
            <PhoneIcon className="w-5 h-5" />
          </button>
          <button
            onClick={handleConversationMenu}
            className="p-2 rounded-lg transition-colors duration-200"
            style={{
              background: "hsl(var(--harbor-surface-1))",
              color: "hsl(var(--harbor-text-secondary))",
            }}
          >
            <EllipsisIcon className="w-5 h-5" />
          </button>
        </div>
      </header>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="max-w-3xl mx-auto space-y-3">
          {selectedConv!.messages.map((message) => (
            <div
              key={message.id}
              className={`flex ${message.isMine ? "justify-end" : "justify-start"}`}
            >
              <div
                className="max-w-[75%] px-4 py-2.5 rounded-2xl"
                style={{
                  background: message.isMine
                    ? "linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))"
                    : "hsl(var(--harbor-surface-1))",
                  color: message.isMine ? "white" : "hsl(var(--harbor-text-primary))",
                  borderBottomRightRadius: message.isMine ? "4px" : "16px",
                  borderBottomLeftRadius: message.isMine ? "16px" : "4px",
                }}
              >
                <p className="text-sm whitespace-pre-wrap">{message.content}</p>
                <p
                  className="text-xs mt-1 text-right"
                  style={{
                    color: message.isMine
                      ? "rgba(255,255,255,0.7)"
                      : "hsl(var(--harbor-text-tertiary))",
                  }}
                >
                  {message.timestamp.toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </p>
              </div>
            </div>
          ))}
          <div ref={messagesEndRef} />
        </div>
      </div>

      {/* Message input */}
      <div
        className="p-4 border-t"
        style={{
          borderColor: "hsl(var(--harbor-border-subtle))",
          background: "hsl(var(--harbor-bg-elevated))",
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
              background: "hsl(var(--harbor-surface-1))",
              border: "1px solid hsl(var(--harbor-border-subtle))",
              color: "hsl(var(--harbor-text-primary))",
            }}
          />
          <button
            onClick={handleSendMessage}
            disabled={!messageInput.trim()}
            className="p-3 rounded-lg transition-all duration-200 flex-shrink-0"
            style={{
              background: messageInput.trim()
                ? "linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))"
                : "hsl(var(--harbor-surface-2))",
              color: messageInput.trim() ? "white" : "hsl(var(--harbor-text-tertiary))",
              boxShadow: messageInput.trim()
                ? "0 4px 12px hsl(var(--harbor-primary) / 0.3)"
                : "none",
            }}
          >
            <SendIcon className="w-5 h-5" />
          </button>
        </div>
        <p
          className="text-xs mt-2 text-center"
          style={{ color: "hsl(var(--harbor-text-tertiary))" }}
        >
          Press Enter to send • {selectedConv?.online ? "Online peers will reply automatically" : "This peer is offline"}
        </p>
      </div>
    </div>
  );
}
