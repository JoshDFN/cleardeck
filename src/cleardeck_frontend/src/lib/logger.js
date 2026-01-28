// Production-safe logging utility
// Only logs in development, silent in production

const isDevelopment = (() => {
  // Check if we're on mainnet IC - if so, NEVER log debug info
  if (typeof window !== 'undefined') {
    const host = window.location.hostname;
    // Production IC domains - no debug logging
    if (host.endsWith('.ic0.app') || host.endsWith('.icp0.io') || host.endsWith('.raw.ic0.app')) {
      return false;
    }
    // Local development
    if (host === 'localhost' || host === '127.0.0.1') {
      return true;
    }
  }
  // Fall back to environment checks - only enable dev logging if explicitly local
  return import.meta.env.DFX_NETWORK === 'local' ||
         import.meta.env.MODE === 'development';
})();

// Error tracking service (can be integrated with Sentry, etc.)
let errorTrackingEnabled = false;

export function initErrorTracking(config = {}) {
  errorTrackingEnabled = config.enabled || false;
  // Future: Initialize Sentry or other error tracking service
}

export const logger = {
  debug: (...args) => {
    if (isDevelopment) {
      console.debug('[DEBUG]', ...args);
    }
  },
  
  log: (...args) => {
    if (isDevelopment) {
      console.log('[LOG]', ...args);
    }
  },
  
  info: (...args) => {
    if (isDevelopment) {
      console.info('[INFO]', ...args);
    }
  },
  
  warn: (...args) => {
    // Always log warnings, but track in production
    if (isDevelopment) {
      console.warn('[WARN]', ...args);
    } else if (errorTrackingEnabled) {
      // Track warnings in production error service
      // trackError('warning', args);
    }
  },
  
  error: (...args) => {
    // Always log errors, and track in production
    console.error('[ERROR]', ...args);
    if (errorTrackingEnabled) {
      // Track errors in production error service
      // trackError('error', args);
    }
  }
};

// Export default logger
export default logger;
