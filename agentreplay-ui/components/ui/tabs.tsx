// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import * as React from "react"

export interface TabsProps {
  defaultValue?: string
  value?: string
  onValueChange?: (value: string) => void
  className?: string
  children?: React.ReactNode
}

export function Tabs({ defaultValue, value, onValueChange, className, children }: TabsProps) {
  const [selectedValue, setSelectedValue] = React.useState(defaultValue || "")

  const currentValue = value !== undefined ? value : selectedValue

  const handleValueChange = (newValue: string) => {
    if (value === undefined) {
      setSelectedValue(newValue)
    }
    onValueChange?.(newValue)
  }

  return (
    <div className={className} data-value={currentValue}>
      {React.Children.map(children, (child) => {
        if (React.isValidElement(child)) {
          return React.cloneElement(child as React.ReactElement<any>, {
            value: currentValue,
            onValueChange: handleValueChange,
          })
        }
        return child
      })}
    </div>
  )
}

export interface TabsListProps {
  className?: string
  children?: React.ReactNode
  value?: string
  onValueChange?: (value: string) => void
}

export function TabsList({ className, children, value, onValueChange }: TabsListProps) {
  return (
    <div className={`inline-flex h-10 items-center justify-center rounded-md bg-surface p-1 text-textSecondary border border-border ${className || ""}`}>
      {React.Children.map(children, (child) => {
        if (React.isValidElement(child)) {
          return React.cloneElement(child as React.ReactElement<any>, {
            currentValue: value,
            onValueChange,
          })
        }
        return child
      })}
    </div>
  )
}

export interface TabsTriggerProps {
  value: string
  className?: string
  children?: React.ReactNode
  currentValue?: string
  onValueChange?: (value: string) => void
}

export function TabsTrigger({ value, className, children, currentValue, onValueChange }: TabsTriggerProps) {
  const isActive = currentValue === value

  return (
    <button
      type="button"
      data-state={isActive ? "active" : "inactive"}
      onClick={() => onValueChange?.(value)}
      className={`inline-flex items-center justify-center whitespace-nowrap rounded-md px-4 py-2 text-sm font-medium ring-offset-background transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 ${
        isActive
          ? "bg-primary text-primary-foreground shadow-md"
          : "text-textSecondary hover:bg-surface-hover hover:text-textPrimary"
      } ${className || ""}`}
    >
      {children}
    </button>
  )
}

export interface TabsContentProps {
  value: string
  className?: string
  children?: React.ReactNode
}

export function TabsContent({ value, className, children }: TabsContentProps) {
  const parentValue = React.useContext(TabsContext)

  if (parentValue !== value) {
    return null
  }

  return (
    <div className={`mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 ${className || ""}`}>
      {children}
    </div>
  )
}

const TabsContext = React.createContext<string>("")
