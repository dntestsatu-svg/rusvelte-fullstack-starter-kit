import { client } from './client';

export type NotificationStatus = 'unread' | 'read';

export interface UserNotification {
	id: string;
	user_id: string;
	notification_type: string;
	title: string;
	body: string;
	related_type?: string | null;
	related_id?: string | null;
	status: NotificationStatus;
	created_at: string;
}

export interface NotificationListResponse {
	notifications: UserNotification[];
	total: number;
	unread_count: number;
	page: number;
	per_page: number;
}

export const notificationsApi = {
	list: async (params: {
		page?: number;
		perPage?: number;
		status?: NotificationStatus;
	}) => {
		const query = new URLSearchParams();
		if (params.page) query.append('page', params.page.toString());
		if (params.perPage) query.append('per_page', params.perPage.toString());
		if (params.status) query.append('status', params.status);

		return client.get<NotificationListResponse>(`/api/v1/notifications?${query.toString()}`);
	},

	markRead: async (notificationId: string) => {
		return client.post<void>(`/api/v1/notifications/${notificationId}/read`);
	}
};
