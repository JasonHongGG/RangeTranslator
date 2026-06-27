import React, { createContext, useContext, useState, useCallback, ReactNode } from 'react'
import { createPortal } from 'react-dom'

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
        <div className="notification-container">
          {notifications.map((notification) => (
            <div 
              key={notification.id} 
              className={`notification-toast notification-${notification.type || 'info'}`}
              onClick={() => removeNotification(notification.id)}
            >
              {notification.message}
            </div>
          ))}
        </div>,
        document.body
      )}
    </NotificationContext.Provider>
  )
}
