use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct PaginationParams {
    pub page: u32,
    pub per_page: u32,
}

impl PaginationParams {
    pub fn normalize(self) -> Self {
        let page = if self.page < 1 { 1 } else { self.page };
        let per_page = if self.per_page < 1 {
            20
        } else if self.per_page > 100 {
            100
        } else {
            self.per_page
        };
        
        Self { page, per_page }
    }

    pub fn offset(&self) -> u64 {
        ((self.page.saturating_sub(1)) as u64) * (self.per_page as u64)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total_count: u64,
    pub total_pages: u32,
    pub page: u32,
    pub per_page: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total_count: u64, params: PaginationParams) -> Self {
        let params = params.normalize();
        let total_pages = (total_count as f64 / params.per_page as f64).ceil() as u32;
        
        Self {
            data,
            total_count,
            total_pages,
            page: params.page,
            per_page: params.per_page,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_normalization() {
        let p = PaginationParams { page: 0, per_page: 0 }.normalize();
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, 20);

        let p2 = PaginationParams { page: 5, per_page: 500 }.normalize();
        assert_eq!(p2.page, 5);
        assert_eq!(p2.per_page, 100);
    }

    #[test]
    fn test_pagination_offset() {
        let p = PaginationParams { page: 1, per_page: 20 };
        assert_eq!(p.offset(), 0);

        let p2 = PaginationParams { page: 2, per_page: 20 };
        assert_eq!(p2.offset(), 20);
    }
}
