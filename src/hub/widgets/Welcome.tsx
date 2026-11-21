import type { WidgetProps } from '../registry';

/** The starter card. It exists so a fresh Hub explains itself; remove it from Customise whenever you like. */
export default function Welcome({ size }: WidgetProps) {
  return (
    <div className="welcome" data-size={size} data-testid="widget-welcome">
      <ul>
        <li><b>Customise</b> to drag cards into a new order, resize them or remove them.</li>
        <li><b>Pin</b> up to four widgets to reach them from the island's Widgets tab.</li>
        <li><b>Add widget</b> lists every widget this version offers.</li>
        {size === 'l' && <li>Everything here stays on this PC. Nothing is sent anywhere.</li>}
      </ul>
    </div>
  );
}
