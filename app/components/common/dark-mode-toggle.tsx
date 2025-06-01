import { useState } from "react"

import { useEffect } from "react"
import { Badge } from "../ui/badge"

export const DarkModeToggle = () => {
  const [isDarkMode, setIsDarkMode] = useState(false)

  useEffect(() => {
    setIsDarkMode(document.documentElement.classList.contains('dark'))
  }, [])

  const toggleDarkMode = () => {
    document.documentElement.classList.toggle('dark')
    setIsDarkMode(!isDarkMode)
  }

  return (
    <div className="flex justify-center items-center gap-2 text-sm cursor-pointer">
      <Badge variant="secondary" onClick={toggleDarkMode}>
        {isDarkMode ? 'Dark Mode' : 'Light Mode'}
      </Badge>
    </div>
  )
}
