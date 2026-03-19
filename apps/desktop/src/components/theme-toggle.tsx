import { Moon, Sun } from "lucide-react";
import { useTheme } from "@/components/theme-provider";
import { Button } from "./ui/button";

type ThemeToggleProps = React.ComponentProps<typeof Button>;

export function ThemeToggle(props: ThemeToggleProps) {
  const { setTheme, theme } = useTheme();

  const handleThemeToggle = () => {
    setTheme(theme === "light" ? "dark" : "light");
  };

  return (
    <Button {...props} onClick={handleThemeToggle} size="icon">
      <Sun className="h-[1.2rem] w-[1.2rem] transition-all dark:scale-0 dark:-rotate-90" />
      <Moon className="absolute h-[1.2rem] w-[1.2rem] transition-all dark:scale-100 dark:rotate-0" />
    </Button>
  );
}
