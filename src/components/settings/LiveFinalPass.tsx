import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface LiveFinalPassProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const LiveFinalPass: React.FC<LiveFinalPassProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const enabled = getSetting("live_final_pass") ?? false;

  return (
    <ToggleSwitch
      checked={enabled}
      onChange={(enabled) => updateSetting("live_final_pass", enabled)}
      isUpdating={isUpdating("live_final_pass")}
      label={t("settings.advanced.liveFinalPass.title")}
      description={t("settings.advanced.liveFinalPass.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
};
