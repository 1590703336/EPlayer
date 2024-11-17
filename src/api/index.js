import axios from "axios";
import md5 from "md5";
let api={}

const BASE_URL = "http://localhost:3000/"; // 本地
//const BASE_URL = "https://eplayer-server.vercel.app/"; // 线上
api = axios.create({

    withCredentials: false, 
    baseURL:BASE_URL+'api'
})

// 添加请求拦截器设置认证头
// api.interceptors.request.use(config => {
//   const token = localStorage.getItem('token'); // 从localStorage获取token
//   if (token) {
//     config.headers.Authorization = `Bearer ${token}`;
//   }
//   return config;
// });

//user
export const getUser = (headers) => api.get(`/user/user`, { headers })
export const getNotebook = (headers) => api.get(`/user/notebook`, { headers }) // success
export const updateUser = (payload, headers) => api.put(`/user/updateUser`,payload, { headers })
//export const updateUserInfo = (payload) => api.post(`/user`,payload)
export const createUser = (payload) => api.post(`/user`,payload)
export const loginUser = (credentials) => api.get(`/user/login?username=${credentials.username}&password=${credentials.password}`) // success
export const updateUserVersion = (payload, headers) => api.put(`/user/updateVersion`, payload, { headers })  // success
export const deleteUser = (id) => api.delete(`/user/${id}`)

//subtitle
export const getSubtitle = (md5) => api.get(`/subtitle/${md5}`)  //叫id还是md5都一样
export const createSubtitle = (payload) => api.post(`/subtitle`,payload)
export const deleteSubtitle = (id) => api.delete(`/subtitle/${id}`)
export const updateSubtitle = (id,payload) => api.put(`/subtitle/${id}`,payload)

const apis={
    getNotebook,
    getUser,
    updateUser,
    //updateUserInfo,
    createUser,
    loginUser,
    updateUserVersion,
    deleteUser,
    getSubtitle,
    createSubtitle,
    deleteSubtitle,
    updateSubtitle
}
export default apis