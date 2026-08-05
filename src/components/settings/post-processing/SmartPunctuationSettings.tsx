import React from "react";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

export const SmartPunctuationSettings: React.FC = React.memo(() => {
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const autoPunctEnabled = getSetting("auto_punctuation_enabled") ?? false;
  const bulletEnabled = getSetting("bullet_points_enabled") ?? false;

  return (
    <>
      <ToggleSwitch
        checked={autoPunctEnabled}
        onChange={(v) => updateSetting("auto_punctuation_enabled", v)}
        isUpdating={isUpdating("auto_punctuation_enabled")}
        label="Auto-punctuation"
        description='Capitalizes sentences, adds trailing periods, infers question marks, and processes spoken commands like "new line" and "comma"'
        descriptionMode="tooltip"
        grouped={true}
      />
      {autoPunctEnabled && (
        <ToggleSwitch
          checked={bulletEnabled}
          onChange={(v) => updateSetting("bullet_points_enabled", v)}
          isUpdating={isUpdating("bullet_points_enabled")}
          label="Bullet point commands"
          description='Say "bullet point" or "new bullet" to insert a • list item'
          descriptionMode="tooltip"
          grouped={true}
        />
      )}
    </>
  );
});

SmartPunctuationSettings.displayName = "SmartPunctuationSettings";
