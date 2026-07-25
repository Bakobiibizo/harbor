import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './App.css';

function logWebRtcRuntimeCapabilities() {
  console.info(
    '[harbor-webrtc-diagnostic]',
    JSON.stringify({
      hasRTCPeerConnection: typeof RTCPeerConnection !== 'undefined',
      hasMediaDevices: typeof navigator.mediaDevices !== 'undefined',
      hasGetUserMedia: typeof navigator.mediaDevices?.getUserMedia === 'function',
    }),
  );
}

window.setTimeout(logWebRtcRuntimeCapabilities, 1_000);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
