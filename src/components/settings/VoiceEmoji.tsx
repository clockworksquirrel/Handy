import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface VoiceEmojiProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const VoiceEmoji: React.FC<VoiceEmojiProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("voice_emoji_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("voice_emoji_enabled", nextEnabled)
        }
        isUpdating={isUpdating("voice_emoji_enabled")}
        label={t("settings.advanced.voiceEmoji.title")}
        description={t("settings.advanced.voiceEmoji.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
