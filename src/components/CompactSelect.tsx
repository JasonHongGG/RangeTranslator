import {
  type ReactNode,
  useEffect,
  useId,
  useRef,
  useState,
} from 'react'
import { FiChevronDown, FiCheck } from "react-icons/fi";

type SelectOption = {
  code: string
  label: string
  nativeLabel: string
}

export function CompactSelect({
  label,
  icon,
  value,
  options,
  disabled,
  onChange,
  menuSide = 'bottom',
}: {
  label: string
  icon: ReactNode
  value: string
  options: ReadonlyArray<SelectOption>
  disabled?: boolean
  onChange: (value: string) => void
  menuSide?: 'top' | 'bottom'
}) {
  const buttonId = useId()
  const menuId = useId()
  const rootRef = useRef<HTMLDivElement | null>(null)
  const [open, setOpen] = useState(false)
  const activeOption = options.find((option) => option.code === value) ?? options[0]

  useEffect(() => {
    if (!open) {
      return
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false)
      }
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false)
      }
    }

    window.addEventListener('pointerdown', handlePointerDown)
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('pointerdown', handlePointerDown)
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  useEffect(() => {
    if (disabled && open) {
      setOpen(false)
    }
  }, [disabled, open])

  return (
    <div
      ref={rootRef}
      className={`compact-select compact-select-${menuSide} ${
        open ? 'compact-select-open' : ''
      }`}
      data-no-drag="true"
    >
      <button
        id={buttonId}
        type="button"
        className="compact-select-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={menuId}
        disabled={disabled}
        onClick={() => setOpen((current) => !current)}
      >
        <span className="compact-select-icon">{icon}</span>
        <span className="compact-select-label">{label}</span>
        <span className="compact-select-current">{activeOption?.nativeLabel ?? value}</span>
        <span className="compact-select-caret">
          <FiChevronDown />
        </span>
      </button>

      {open ? (
        <div
          id={menuId}
          className={`compact-select-menu compact-select-menu-${menuSide}`}
          role="listbox"
          aria-labelledby={buttonId}
        >
          {options.map((option) => {
            const active = option.code === value
            return (
              <button
                key={option.code}
                type="button"
                className={`compact-select-option ${
                  active ? 'compact-select-option-active' : ''
                }`}
                role="option"
                aria-selected={active}
                onClick={() => {
                  onChange(option.code)
                  setOpen(false)
                }}
              >
                <span className="compact-select-option-copy">
                  <span className="compact-select-option-primary">
                    {option.nativeLabel}
                  </span>
                  <span className="compact-select-option-secondary">{option.label}</span>
                </span>

                {active ? (
                  <span className="compact-select-option-check">
                    <FiCheck />
                  </span>
                ) : null}
              </button>
            )
          })}
        </div>
      ) : null}
    </div>
  )
}
