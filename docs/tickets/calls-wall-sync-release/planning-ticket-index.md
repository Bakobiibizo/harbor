---
ldgr_doc: 1
kind: ticket_index
id: ticket_index.calls-wall-sync-release.v1
schema: ldgr.ticket_index.v1
status: ready
tags:
- harbor
- ticket-index
---

# Calls, Video, and Wall Sync Ticket Index

This index references ticket projection artifacts recorded in the parent Harbor LDGR run.

```ldgr-ticket-index yaml
tickets:
- id: ticket.0001-reconcile-release-capability-contract
  artifact: artifact:7
  title: "Reconcile Harbor release capability contract"
  work_item: work:0001-reconcile-release-capability-contract
- id: ticket.0101-calling-signaling-transport
  artifact: artifact:8
  title: "Implement signed calling signaling transport"
  work_item: work:0101-calling-signaling-transport
- id: ticket.0102-call-session-state-history
  artifact: artifact:9
  title: "Persist call session state and history"
  work_item: work:0102-call-session-state-history
- id: ticket.0103-webrtc-audio-runtime
  artifact: artifact:10
  title: "Implement 1:1 WebRTC audio runtime"
  work_item: work:0103-webrtc-audio-runtime
- id: ticket.0104-calling-ui-and-events
  artifact: artifact:11
  title: "Add production call UI and event handling"
  work_item: work:0104-calling-ui-and-events
- id: ticket.0105-call-ice-nat-configuration
  artifact: artifact:12
  title: "Implement ICE, STUN, and TURN configuration for calls"
  work_item: work:0105-call-ice-nat-configuration
- id: ticket.0106-voice-call-integration-validation
  artifact: artifact:13
  title: "Validate end-to-end 1:1 voice calling"
  work_item: work:0106-voice-call-integration-validation
- id: ticket.0201-group-call-topology-contract
  artifact: artifact:14
  title: "Select and document production group-call topology"
  work_item: work:0201-group-call-topology-contract
- id: ticket.0202-video-call-media-runtime
  artifact: artifact:15
  title: "Implement one-to-one video call runtime"
  work_item: work:0202-video-call-media-runtime
- id: ticket.0203-group-call-signaling-membership
  artifact: artifact:16
  title: "Implement group-call signaling and membership control"
  work_item: work:0203-group-call-signaling-membership
- id: ticket.0204-group-call-media-layout-runtime
  artifact: artifact:17
  title: "Implement group call media runtime and UI"
  work_item: work:0204-group-call-media-layout-runtime
- id: ticket.0205-video-group-call-validation
  artifact: artifact:18
  title: "Validate video and group calling release readiness"
  work_item: work:0205-video-group-call-validation
- id: ticket.0301-wall-author-visibility-settings
  artifact: artifact:19
  title: "Implement wall author visibility controls"
  work_item: work:0301-wall-author-visibility-settings
- id: ticket.0302-wall-media-signature-integrity
  artifact: artifact:20
  title: "Bind wall media to signed post integrity"
  work_item: work:0302-wall-media-signature-integrity
- id: ticket.0303-wall-edit-delete-sync
  artifact: artifact:21
  title: "Synchronize wall edits and deletes"
  work_item: work:0303-wall-edit-delete-sync
- id: ticket.0304-wall-author-social-ui
  artifact: artifact:22
  title: "Show real comments and reactions on author wall"
  work_item: work:0304-wall-author-social-ui
- id: ticket.0305-wall-preview-rss-share-ui
  artifact: artifact:23
  title: "Expose wall preview, RSS, and share surfaces"
  work_item: work:0305-wall-preview-rss-share-ui
- id: ticket.0401-contact-wall-view
  artifact: artifact:24
  title: "Implement contact wall consumer view"
  work_item: work:0401-contact-wall-view
- id: ticket.0402-feed-interactions-real
  artifact: artifact:25
  title: "Replace feed placeholder interactions with durable behavior"
  work_item: work:0402-feed-interactions-real
- id: ticket.0403-consumer-comments-reactions-ui
  artifact: artifact:26
  title: "Implement consumer comments and reactions on feed/contact walls"
  work_item: work:0403-consumer-comments-reactions-ui
- id: ticket.0404-consumer-media-fetch-lifecycle
  artifact: artifact:27
  title: "Harden consumer media fetching and rendering lifecycle"
  work_item: work:0404-consumer-media-fetch-lifecycle
- id: ticket.0501-relay-wall-permission-enforcement
  artifact: artifact:28
  title: "Enforce wall visibility and permissions through relay sync"
  work_item: work:0501-relay-wall-permission-enforcement
- id: ticket.0502-wall-sync-cursors-pagination
  artifact: artifact:29
  title: "Implement durable wall sync cursors and pagination"
  work_item: work:0502-wall-sync-cursors-pagination
- id: ticket.0503-wall-event-reconciliation-tombstones
  artifact: artifact:30
  title: "Implement wall event reconciliation and tombstones"
  work_item: work:0503-wall-event-reconciliation-tombstones
- id: ticket.0504-wall-social-event-model
  artifact: artifact:31
  title: "Implement signed wall social event model"
  work_item: work:0504-wall-social-event-model
- id: ticket.0505-wall-sync-status-observability
  artifact: artifact:32
  title: "Expose wall sync status and observability"
  work_item: work:0505-wall-sync-status-observability
- id: ticket.0506-wall-sync-multi-profile-validation
  artifact: artifact:33
  title: "Validate host/consumer wall synchronization"
  work_item: work:0506-wall-sync-multi-profile-validation
- id: ticket.0601-docs-and-release-gates-for-calls-wall-sync
  artifact: artifact:34
  title: "Update docs and release gates for calls and wall sync"
  work_item: work:0601-docs-and-release-gates-for-calls-wall-sync
```
