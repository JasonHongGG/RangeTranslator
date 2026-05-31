import {
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
  value,
  options,
  disabled,
  onChange,
  menuSide = 'bottom',
}: {
  value: string
  options: ReadonlyArray<SelectOption>
  disabled?: boolean
  onChange: (value: string) => void
  menuSide?: 'top' | 'bottom'
}) {
  return (
    <CompactSelectInner
      key={disabled ? 'disabled' : 'enabled'}
      value={value}
      options={options}
      disabled={disabled}
      onChange={onChange}
      menuSide={menuSide}
    />
  )
}

function CompactSelectInner({
  value,
  options,
  disabled,
  onChange,
  menuSide = 'bottom',
}: {
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
    if (!open) return

    const handlePointerDown = (event: PointerEvent) => {
      // Use composedPath to correctly handle clicks inside the scrollbar on Windows
      if (rootRef.current && !event.composedPath().includes(rootRef.current)) {
        setOpen(false)
      }
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }

    window.addEventListener('pointerdown', handlePointerDown)
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('pointerdown', handlePointerDown)
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  return (
    <div
      ref={rootRef}
      className={`custom-select ${open ? 'open' : ''}`}
      data-no-drag="true"
    >
      <button
        id={buttonId}
        type="button"
        className={`select-trigger ${open ? 'expanded' : ''}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={menuId}
        disabled={disabled}
        onClick={() => setOpen((current) => !current)}
      >
        <span style={{flex: 1, textAlign: 'center'}}>{activeOption?.nativeLabel ?? value}</span>
        <FiChevronDown className="select-caret" style={{ transform: open ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'transform 0.2s ease' }} />
      </button>

      {open ? (
        <div
          id={menuId}
          className={`select-menu select-menu-${menuSide}`}
          role="listbox"
          aria-labelledby={buttonId}
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          {options.map((option) => {
            const active = option.code === value
            return (
              <button
                key={option.code}
                type="button"
                className={`select-option ${active ? 'active' : ''}`}
                role="option"
                aria-selected={active}
                onClick={() => {
                  onChange(option.code)
                  setOpen(false)
                }}
              >
                <span>{option.nativeLabel}</span>
                {active && <FiCheck className="select-option-check" />}
              </button>
            )
          })}
        </div>
      ) : null}
    </div>
  )
}
