import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface AlwaysShowOverlayProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AlwaysShowOverlay: React.FC<AlwaysShowOverlayProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("overlay_always_visible") ?? false;
    // The idle pill is part of the overlay, so it has no effect when the overlay is off.
    const overlayOff = getSetting("overlay_style") === "none";

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("overlay_always_visible", value)}
        isUpdating={isUpdating("overlay_always_visible")}
        disabled={overlayOff}
        label={t("settings.general.alwaysShowOverlay.title")}
        description={t("settings.general.alwaysShowOverlay.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
