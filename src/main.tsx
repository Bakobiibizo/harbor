import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './App.css';
import { installGlobalErrorHandlers } from './utils/globalErrorHandler';

// Install global handlers for uncaught errors and unhandled promise rejections.
// This must run before React renders so that early async failures are captured.
installGlobalErrorHandlers();

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
