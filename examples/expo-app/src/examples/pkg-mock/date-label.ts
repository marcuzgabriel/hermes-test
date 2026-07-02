import moment from 'moment';

export function dateLabel(iso: string): string {
  return `label: ${moment(iso).format('YYYY')}`;
}
