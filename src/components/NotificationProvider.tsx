import { createContext, useContext, useState, useCallback } from 'react'
import type { ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { motion, AnimatePresence } from 'framer-motion'
import { FiInfo, FiCheckCircle, FiAlertCircle, FiXCircle, FiX } from 'react-icons/fi'

import './NotificationProvider.css'

export type NotificationType = 'info' | 'success' | 'error' | 'warning'

export interface NotificationOptions {
  message: string
  type?: NotificationType
  duration?: number
}

interface NotificationContextType {
  showNotification: (options: NotificationOptions | string) => void
}

const NotificationContext = createContext<NotificationContextType | undefined>(undefined)

// eslint-disable-next-line react-refresh/only-export-components
export function useNotification() {
  const context = useContext(NotificationContext)
  if (!context) {
    throw new Error('useNotification must be used within a NotificationProvider')
  }
  return context
}

interface NotificationItem extends NotificationOptions {
  id: string
}

const ICONS = {
  info: <FiInfo className="notif-icon info" />,
  success: <FiCheckCircle className="notif-icon success" />,
  error: <FiXCircle className="notif-icon error" />,
  warning: <FiAlertCircle className="notif-icon warning" />,
}

export function NotificationProvider({ children }: { children: ReactNode }) {
  const [notifications, setNotifications] = useState<NotificationItem[]>([])

  const showNotification = useCallback((options: NotificationOptions | string) => {
    const id = Math.random().toString(36).substring(2, 9)
    
    let notificationOpts: NotificationOptions
    if (typeof options === 'string') {
      notificationOpts = { message: options, type: 'info', duration: 3000 }
    } else {
      notificationOpts = { duration: 3000, type: 'info', ...options }
    }

    // Log to console based on type so errors appear in DevTools
    if (notificationOpts.type === 'error') {
      console.error(`[Notification Error]:`, notificationOpts.message)
    } else if (notificationOpts.type === 'warning') {
      console.warn(`[Notification Warning]:`, notificationOpts.message)
    } else {
      console.log(`[Notification ${notificationOpts.type}]:`, notificationOpts.message)
    }

    setNotifications((prev) => [...prev, { id, ...notificationOpts }])

    if (notificationOpts.duration && notificationOpts.duration > 0) {
      setTimeout(() => {
        setNotifications((prev) => prev.filter((n) => n.id !== id))
      }, notificationOpts.duration)
    }
  }, [])

  const removeNotification = useCallback((id: string) => {
    setNotifications((prev) => prev.filter((n) => n.id !== id))
  }, [])

  return (
    <NotificationContext.Provider value={{ showNotification }}>
      {children}
      {createPortal(
        <div className="v2-notification-container">
          <AnimatePresence>
            {notifications.map((notification) => (
              <motion.div 
                key={notification.id}
                layout
                initial={{ opacity: 0, y: -20, scale: 0.95 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95, transition: { duration: 0.2 } }}
                className={`v2-notification-toast type-${notification.type || 'info'}`}
              >
                <div className="v2-notification-icon-wrapper">
                  {ICONS[notification.type || 'info']}
                </div>
                <div className="v2-notification-content">
                  {notification.message}
                </div>
                <button 
                  type="button"
                  className="v2-notification-close"
                  onClick={() => removeNotification(notification.id)}
                >
                  <FiX />
                </button>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>,
        document.body
      )}
    </NotificationContext.Provider>
  )
}
